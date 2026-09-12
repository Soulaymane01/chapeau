use adw::prelude::*;
use chapeau::core::ResourceType;
use chapeau::services::detail::ResourceDetail;
use gtk4 as gtk;
use libadwaita as adw;

/// Maximum names shown in a relationship group.
const LIST_LIMIT: usize = 10;

/// A resource detail page, populated on demand.
pub struct DetailPage {
    pub page: adw::NavigationPage,
    header: adw::HeaderBar,
    body: gtk::Box,
}

impl DetailPage {
    pub fn new() -> Self {
        let header = adw::HeaderBar::new();
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(18)
            .margin_bottom(24)
            .margin_start(18)
            .margin_end(18)
            .build();
        let clamp = adw::Clamp::builder().maximum_size(760).child(&body).build();
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&clamp)
            .vexpand(true)
            .build();
        let toolbar = adw::ToolbarView::builder().content(&scrolled).build();
        toolbar.add_top_bar(&header);
        let page = adw::NavigationPage::builder()
            .title("Resource")
            .child(&toolbar)
            .build();

        Self { page, header, body }
    }

    /// Show a loading state while the worker fetches the detail.
    pub fn set_loading(&self, native_id: &str) {
        self.clear();
        self.page.set_title(native_id);
        self.header
            .set_title_widget(Some(&adw::WindowTitle::new(native_id, "Loading…")));

        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Center)
            .margin_top(48)
            .build();
        self.body.append(&spinner);
    }

    fn clear(&self) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }

    pub fn populate(&self, detail: &ResourceDetail) {
        self.clear();

        let resource = &detail.resource;
        let title = resource
            .display_name
            .clone()
            .unwrap_or_else(|| resource.native_id.clone());
        self.page.set_title(&title);
        self.header
            .set_title_widget(Some(&adw::WindowTitle::new(&title, &resource.native_id)));

        if detail.is_missing() && detail.root.is_some() {
            let banner = adw::Banner::builder()
                .title("Recorded as intentional, but not currently present on the system")
                .build();
            self.body.append(&banner);
        }

        if let Some(summary) = detail.metadata_str("summary") {
            let label = gtk::Label::builder()
                .label(summary)
                .wrap(true)
                .halign(gtk::Align::Start)
                .css_classes(["dim-label"])
                .build();
            self.body.append(&label);
        }

        let facts = adw::PreferencesGroup::builder().title("Facts").build();
        facts.add(&fact("Type", &resource.resource_type.to_string()));
        match resource.resource_type {
            ResourceType::Package => {
                if let Some(version) = detail
                    .observation
                    .as_ref()
                    .and_then(|obs| obs.version.as_deref())
                {
                    let value = if detail.is_missing() {
                        format!("{version} (last seen)")
                    } else {
                        version.to_string()
                    };
                    facts.add(&fact("Version", &value));
                }
                facts.add(&fact("Recorded", &recorded_text(detail)));
            }
            ResourceType::Service => {
                facts.add(&fact("State", &service_text(detail)));
            }
            ResourceType::Flatpak => {
                if let Some(version) = detail
                    .observation
                    .as_ref()
                    .and_then(|obs| obs.version.as_deref())
                {
                    facts.add(&fact("Version", version));
                }
                if let Some(branch) = detail.metadata_str("branch") {
                    facts.add(&fact("Branch", branch));
                }
                facts.add(&fact("Recorded", &recorded_text(detail)));
            }
            ResourceType::Repository => {
                if let Some(count) = detail.package_count {
                    facts.add(&fact("Packages", &count.to_string()));
                }
            }
        }
        if !detail.provenance.is_empty() {
            facts.add(&fact("Origin", &detail.provenance.join(", ")));
        }
        facts.add(&fact("Intent", &intent_text(detail)));
        if !detail.domains.is_empty() {
            let memberships = detail
                .domains
                .iter()
                .map(|(domain, relationship)| format!("{} ({})", domain.name, relationship))
                .collect::<Vec<_>>()
                .join(", ");
            facts.add(&fact("Domains", &memberships));
        }
        self.body.append(&facts);

        name_group(&self.body, "Dependencies", &detail.dependencies);
        name_group(&self.body, "Required by", &detail.dependents);
        name_group(&self.body, "Uses", &detail.uses);
    }
}

fn fact(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build()
}

fn name_group(body: &gtk::Box, title: &str, names: &[String]) {
    if names.is_empty() {
        return;
    }

    let group = adw::PreferencesGroup::builder()
        .title(format!("{} ({})", title, names.len()))
        .build();
    for name in names.iter().take(LIST_LIMIT) {
        group.add(&adw::ActionRow::builder().title(name).build());
    }
    if names.len() > LIST_LIMIT {
        let more = gtk::Label::builder()
            .label(format!("… and {} more", names.len() - LIST_LIMIT))
            .halign(gtk::Align::Start)
            .css_classes(["dim-label"])
            .build();
        group.add(&more);
    }
    body.append(&group);
}

fn recorded_text(detail: &ResourceDetail) -> String {
    match detail.observation.as_ref().and_then(|obs| obs.installed) {
        Some(true) => "installed".to_string(),
        Some(false) => "missing — not currently present on the system".to_string(),
        None => "not recorded".to_string(),
    }
}

fn service_text(detail: &ResourceDetail) -> String {
    let Some(observation) = detail.observation.as_ref() else {
        return "unknown".to_string();
    };

    let mut parts = Vec::new();
    match observation.active {
        Some(true) => parts.push("active"),
        Some(false) => parts.push("inactive"),
        None => {}
    }
    match observation.enabled {
        Some(true) => parts.push("enabled"),
        Some(false) => parts.push("disabled"),
        None => {}
    }
    if observation.failed == Some(true) {
        parts.push("failed");
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(", ")
    }
}

fn intent_text(detail: &ResourceDetail) -> String {
    match &detail.root {
        Some(root) => match &root.reason {
            Some(reason) => format!("intentional ({}) — {reason}", root.source),
            None => format!("intentional ({})", root.source),
        },
        None => match detail.metadata_str("role") {
            Some(role) => format!("not a root (role: {role})"),
            None => "not a root".to_string(),
        },
    }
}
