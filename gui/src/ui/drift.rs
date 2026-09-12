use adw::prelude::*;
use chapeau::services::drift::{DriftEntry, DriftReport};
use gtk4 as gtk;
use libadwaita as adw;

/// Read-only drift dashboard with a one-click reconcile.
pub struct DriftPage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
}

impl DriftPage {
    pub fn new<F: Fn() + 'static>(on_reconcile: F) -> Self {
        let header = adw::HeaderBar::new();
        let reconcile = gtk::Button::builder()
            .label("Reconcile")
            .tooltip_text("Run a scan to bring Chapeau back in sync")
            .css_classes(["suggested-action"])
            .build();
        reconcile.connect_clicked(move |_| on_reconcile());
        header.pack_end(&reconcile);

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
            .title("Drift")
            .child(&toolbar)
            .build();

        Self { page, body }
    }

    pub fn set_loading(&self) {
        self.clear();
        self.body.append(&spinner());
    }

    fn clear(&self) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }

    pub fn populate(&self, report: &DriftReport) {
        self.clear();

        if report.is_empty() {
            let status = adw::StatusPage::builder()
                .icon_name("emblem-ok-symbolic")
                .title("No drift detected")
                .description("Chapeau's recorded state matches the live system.")
                .margin_top(48)
                .build();
            self.body.append(&status);
            return;
        }

        self.body.append(
            &adw::Banner::builder()
                .title("Reconcile to update Chapeau's recorded state")
                .build(),
        );

        append_group(&self.body, "Missing", &report.missing);
        append_group(&self.body, "New", &report.added);
        append_group(&self.body, "Changed", &report.changed);
    }
}

fn append_group(body: &gtk::Box, title: &str, entries: &[DriftEntry]) {
    if entries.is_empty() {
        return;
    }

    let group = adw::PreferencesGroup::builder()
        .title(format!("{} ({})", title, entries.len()))
        .build();
    for entry in entries {
        let subtitle = format!("{} · {}", entry.resource_type, entry_subtitle(entry));
        group.add(
            &adw::ActionRow::builder()
                .title(&entry.name)
                .subtitle(subtitle)
                .build(),
        );
    }
    body.append(&group);
}

fn entry_subtitle(entry: &DriftEntry) -> String {
    match (entry.recorded.as_deref(), entry.actual.as_deref()) {
        (Some(recorded), Some(actual)) => format!("{} → {}", recorded, actual),
        (Some(recorded), None) => format!("last seen {}", recorded),
        (None, Some(actual)) => actual.to_string(),
        (None, None) => "no recorded state".to_string(),
    }
}

fn spinner() -> gtk::Spinner {
    gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .margin_top(48)
        .build()
}
