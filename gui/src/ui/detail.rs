use adw::prelude::*;
use chapeau::core::ResourceType;
use chapeau::services::detail::ResourceDetail;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// Maximum names shown in a relationship group.
const LIST_LIMIT: usize = 10;

type AddDomainCallback = Rc<dyn Fn(String, String, bool)>;
type RemoveDomainCallback = Rc<dyn Fn(String, String)>;
type RemoveCallback = Rc<dyn Fn(String)>;

/// A resource detail page, populated on demand.
pub struct DetailPage {
    pub page: adw::NavigationPage,
    header: adw::HeaderBar,
    body: gtk::Box,
    window: adw::ApplicationWindow,
    on_add_domain: AddDomainCallback,
    on_remove_domain: RemoveDomainCallback,
    on_remove: RemoveCallback,
    current: RefCell<Option<String>>,
}

impl DetailPage {
    pub fn new<FA, FR, FX>(
        window: &adw::ApplicationWindow,
        on_add_domain: FA,
        on_remove_domain: FR,
        on_remove: FX,
    ) -> Self
    where
        FA: Fn(String, String, bool) + 'static,
        FR: Fn(String, String) + 'static,
        FX: Fn(String) + 'static,
    {
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

        Self {
            page,
            header,
            body,
            window: window.clone(),
            on_add_domain: Rc::new(on_add_domain),
            on_remove_domain: Rc::new(on_remove_domain),
            on_remove: Rc::new(on_remove),
            current: RefCell::new(None),
        }
    }

    /// The native id of the resource currently shown, if any.
    pub fn current_native_id(&self) -> Option<String> {
        self.current.borrow().clone()
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
        *self.current.borrow_mut() = Some(detail.resource.native_id.clone());

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
        self.body.append(&facts);

        self.append_domains_group(detail);

        name_group(&self.body, "Dependencies", &detail.dependencies);
        name_group(&self.body, "Required by", &detail.dependents);
        name_group(&self.body, "Uses", &detail.uses);

        if resource.resource_type == ResourceType::Package {
            self.append_remove_group(&resource.native_id);
        }
    }

    /// Destructive action: preview and remove a package.
    fn append_remove_group(&self, native_id: &str) {
        let group = adw::PreferencesGroup::builder().title("Actions").build();
        let row = adw::ActionRow::builder()
            .title("Remove package…")
            .subtitle("Preview the impact before anything happens")
            .activatable(true)
            .css_classes(["error"])
            .build();

        let on_remove = self.on_remove.clone();
        let native_id = native_id.to_string();
        row.connect_activated(move |_| on_remove(native_id.clone()));

        group.add(&row);
        self.body.append(&group);
    }

    /// Domain memberships with per-row removal and an "add to domain" action.
    fn append_domains_group(&self, detail: &ResourceDetail) {
        let resource_id = detail.resource.native_id.clone();
        let domains = Rc::new(detail.all_domains.clone());

        let group = adw::PreferencesGroup::builder()
            .title(format!("Domains ({})", detail.domains.len()))
            .build();

        for (domain, relationship) in &detail.domains {
            let row = adw::ActionRow::builder()
                .title(&domain.name)
                .subtitle(relationship.to_string())
                .build();

            let remove = gtk::Button::builder()
                .icon_name("list-remove-symbolic")
                .tooltip_text("Remove from domain")
                .css_classes(["flat"])
                .build();
            {
                let resource_id = resource_id.clone();
                let domain_name = domain.name.clone();
                let on_remove = self.on_remove_domain.clone();
                remove.connect_clicked(move |_| {
                    on_remove(resource_id.clone(), domain_name.clone());
                });
            }
            row.add_suffix(&remove);
            group.add(&row);
        }

        let add_row = adw::ActionRow::builder().title("Add to domain").build();
        if domains.is_empty() {
            add_row.set_subtitle("Create a domain first (menu → Domains)");
        } else {
            let add = gtk::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text("Add to a domain")
                .css_classes(["flat"])
                .build();
            {
                let window = self.window.clone();
                let domains = domains.clone();
                let resource_id = resource_id.clone();
                let on_add = self.on_add_domain.clone();
                let open = move || add_domain_dialog(&window, &domains, &resource_id, &on_add);
                add.connect_clicked(move |_| open());
            }
            add_row.add_suffix(&add);
            add_row.set_activatable(true);
            {
                let window = self.window.clone();
                let domains = domains.clone();
                let resource_id = resource_id.clone();
                let on_add = self.on_add_domain.clone();
                add_row.connect_activated(move |_| {
                    add_domain_dialog(&window, &domains, &resource_id, &on_add)
                });
            }
        }
        group.add(&add_row);

        self.body.append(&group);
    }
}

fn add_domain_dialog(
    window: &adw::ApplicationWindow,
    domains: &[String],
    resource_id: &str,
    on_add: &AddDomainCallback,
) {
    let labels: Vec<&str> = domains.iter().map(|name| name.as_str()).collect();
    let dropdown = gtk::DropDown::from_strings(&labels);
    let owns = gtk::CheckButton::with_label("Owns this resource (authoritative)");
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();
    content.append(&dropdown);
    content.append(&owns);

    let dialog = adw::AlertDialog::builder()
        .heading("Add to domain")
        .extra_child(&content)
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("add", "Add");
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("add"));
    dialog.set_close_response("cancel");

    let on_add = on_add.clone();
    let resource_id = resource_id.to_string();
    let domains: Vec<String> = domains.to_vec();
    dialog.connect_response(None, move |_, response| {
        if response != "add" {
            return;
        }
        let index = dropdown.selected() as usize;
        if let Some(domain) = domains.get(index) {
            on_add(resource_id.clone(), domain.clone(), owns.is_active());
        }
    });

    dialog.present(Some(window));
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
