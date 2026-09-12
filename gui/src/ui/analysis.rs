use adw::prelude::*;
use chapeau::services::analysis::{AnalysisKind, ResourceAnalysis};
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;

/// Maximum entries rendered; the CLI lists the complete set.
const ENTRY_LIMIT: usize = 100;

/// Cleanup analysis: potentially orphaned or unused resources.
pub struct AnalysisPage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
    kind: Rc<Cell<AnalysisKind>>,
    dropdown: gtk::DropDown,
    syncing: Rc<Cell<bool>>,
}

impl AnalysisPage {
    pub fn new<F: Fn(AnalysisKind) + 'static>(on_change: F) -> Self {
        let dropdown = gtk::DropDown::from_strings(&[
            AnalysisKind::Orphaned.title(),
            AnalysisKind::Unused.title(),
        ]);
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&dropdown));

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
            .title(AnalysisKind::Orphaned.title())
            .tag("analysis")
            .child(&toolbar)
            .build();

        let kind = Rc::new(Cell::new(AnalysisKind::Orphaned));
        let syncing = Rc::new(Cell::new(false));
        {
            let kind = kind.clone();
            let syncing = syncing.clone();
            let on_change = Rc::new(on_change);
            dropdown.connect_selected_notify(move |dropdown| {
                if syncing.get() {
                    return;
                }
                let selected = match dropdown.selected() {
                    1 => AnalysisKind::Unused,
                    _ => AnalysisKind::Orphaned,
                };
                kind.set(selected);
                on_change(selected);
            });
        }

        Self {
            page,
            body,
            kind,
            dropdown,
            syncing,
        }
    }

    pub fn set_loading(&self, kind: AnalysisKind) {
        self.kind.set(kind);
        self.page.set_title(kind.title());
        self.syncing.set(true);
        self.dropdown.set_selected(match kind {
            AnalysisKind::Orphaned => 0,
            AnalysisKind::Unused => 1,
        });
        self.syncing.set(false);
        self.clear();
        self.body
            .append(&gtk::Spinner::builder().spinning(true).build());
    }

    fn clear(&self) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }

    pub fn populate(&self, kind: AnalysisKind, entries: &[ResourceAnalysis]) {
        self.kind.set(kind);
        self.page
            .set_title(&format!("{} ({})", kind.title(), entries.len()));
        self.clear();

        if entries.is_empty() {
            let status = adw::StatusPage::builder()
                .icon_name("emblem-ok-symbolic")
                .title("Nothing found")
                .description(kind.description())
                .margin_top(48)
                .build();
            self.body.append(&status);
            return;
        }

        self.body.append(
            &adw::Banner::builder()
                .title("Heuristics, not proof — nothing is removed automatically")
                .build(),
        );

        let group = adw::PreferencesGroup::builder().title(kind.title()).build();
        for entry in entries.iter().take(ENTRY_LIMIT) {
            group.add(&entry_expander(entry));
        }
        if entries.len() > ENTRY_LIMIT {
            group.add(
                &gtk::Label::builder()
                    .label(format!(
                        "… and {} more (use the CLI for the complete list)",
                        entries.len() - ENTRY_LIMIT
                    ))
                    .halign(gtk::Align::Start)
                    .css_classes(["dim-label"])
                    .build(),
            );
        }
        self.body.append(&group);
    }
}

fn entry_expander(entry: &ResourceAnalysis) -> adw::ExpanderRow {
    let resource = &entry.resource;
    let label = resource
        .display_name
        .clone()
        .unwrap_or_else(|| resource.native_id.clone());
    let reasons = entry
        .reasons
        .iter()
        .map(|reason| reason.description())
        .collect::<Vec<_>>()
        .join(", ");

    let expander = adw::ExpanderRow::builder()
        .use_markup(false)
        .title(&label)
        .subtitle(format!("{} · {}", resource.resource_type, reasons))
        .build();

    for reason in &entry.reasons {
        expander.add_row(
            &adw::ActionRow::builder()
                .use_markup(false)
                .title(reason.description())
                .build(),
        );
    }
    add_list(&expander, "Still required by", &entry.dependents);
    add_list(&expander, "Owned by domains", &entry.owned_by_domains);
    add_list(&expander, "Used by domains", &entry.used_by_domains);
    add_list(&expander, "Affected services", &entry.affected_services);

    expander
}

fn add_list(expander: &adw::ExpanderRow, title: &str, names: &[String]) {
    if names.is_empty() {
        return;
    }
    expander.add_row(
        &adw::ActionRow::builder()
            .use_markup(false)
            .title(title)
            .subtitle(names.join(", "))
            .build(),
    );
}
