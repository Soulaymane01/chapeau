use adw::prelude::*;
use chapeau::services::overview::{Overview, Section};
use gtk4 as gtk;
use libadwaita as adw;
use std::rc::Rc;

/// The My System home page: intentional resources as the main content.
pub struct HomePage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
    on_open: Rc<dyn Fn(String)>,
    status_label: gtk::Label,
}

impl HomePage {
    pub fn new<F: Fn(String) + 'static>(on_open: F) -> Self {
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
            .title("My System")
            .tag("home")
            .child(&toolbar)
            .build();

        let status_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .visible(false)
            .css_classes(["accent"])
            .build();

        Self {
            page,
            body,
            on_open: Rc::new(on_open),
            status_label,
        }
    }

    /// Show or clear a status line above the content (used for scan progress).
    pub fn set_status(&self, text: Option<&str>) {
        match text {
            Some(text) => {
                self.status_label.set_label(text);
                self.status_label.set_visible(true);
                if self.body.first_child().as_ref() != Some(self.status_label.upcast_ref()) {
                    self.body.prepend(&self.status_label);
                }
            }
            None => self.status_label.set_visible(false),
        }
    }

    pub fn populate(&self, view: &Overview) {
        self.clear();
        if self.status_label.is_visible() {
            self.body.append(&self.status_label);
        }

        if view.root_count == 0 {
            self.body.append(
                &adw::StatusPage::builder()
                    .icon_name("system-software-install-symbolic")
                    .title("My System")
                    .description("No intentional resources yet. Click Scan to discover what is installed, or hide resources you do not want to see.")
                    .margin_top(48)
                    .build(),
            );
            return;
        }

        for section in &view.sections {
            self.body.append(&section_group(section, &self.on_open));
        }

        let mut summary = format!("{} intentional resources", view.root_count);
        if view.missing_count > 0 {
            summary.push_str(&format!(" · {} missing", view.missing_count));
        }
        if view.hidden_count > 0 {
            summary.push_str(&format!(" · {} hidden", view.hidden_count));
        }
        summary.push_str(&format!(
            " · {} tracked resources · {} relationships",
            view.resource_count, view.relationship_count
        ));
        let footer = gtk::Label::builder()
            .label(summary)
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        self.body.append(&footer);

        if view.hidden_count > 0 {
            let hint = gtk::Label::builder()
                .label("Hidden resources stay tracked; manage them from a resource page or 'chapeau roots --all'.")
                .halign(gtk::Align::Start)
                .wrap(true)
                .css_classes(["dim-label"])
                .build();
            self.body.append(&hint);
        }
    }

    fn clear(&self) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }
}

fn section_group(section: &Section, on_open: &Rc<dyn Fn(String)>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(format!("{} ({})", section.title, section.entries.len()))
        .build();

    for entry in &section.entries {
        let row = adw::ActionRow::builder()
            .use_markup(false)
            .title(entry.label())
            .subtitle(entry.tag())
            .activatable(true)
            .build();
        let on_open = on_open.clone();
        let native_id = entry.resource.native_id.clone();
        row.connect_activated(move |_| on_open(native_id.clone()));
        group.add(&row);
    }

    group
}
