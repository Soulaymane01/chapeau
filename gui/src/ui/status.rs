use adw::prelude::*;
use chapeau::services::status::StatusSummary;
use gtk4 as gtk;
use libadwaita as adw;

/// System status dashboard.
pub struct StatusPage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
}

impl StatusPage {
    pub fn new<F: Fn() + 'static>(on_refresh: F) -> Self {
        let header = adw::HeaderBar::new();
        let refresh = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh status")
            .build();
        refresh.connect_clicked(move |_| on_refresh());
        header.pack_end(&refresh);

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
            .title("Status")
            .tag("status")
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

    pub fn populate(&self, summary: &StatusSummary) {
        self.clear();

        let resources = adw::PreferencesGroup::builder().title("Resources").build();
        resources.add(&row("Total", &summary.total_resources.to_string()));
        resources.add(&row("Packages", &summary.packages.to_string()));
        resources.add(&row("Services", &summary.services.to_string()));
        resources.add(&row("Flatpaks", &summary.flatpaks.to_string()));
        resources.add(&row("Repositories", &summary.repositories.to_string()));
        self.body.append(&resources);

        let model = adw::PreferencesGroup::builder()
            .title("Chapeau Model")
            .build();
        model.add(&row("Domains", &summary.domains.to_string()));
        model.add(&row("Untracked", &summary.untracked.to_string()));
        model.add(&row("Missing", &summary.missing.to_string()));
        model.add(&row("Relationships", &summary.relationships.to_string()));
        model.add(&row("Observations", &summary.observations.to_string()));
        self.body.append(&model);

        let services = adw::PreferencesGroup::builder().title("Services").build();
        services.add(&row("Active", &summary.active_services.to_string()));
        services.add(&row("Enabled", &summary.enabled_services.to_string()));
        services.add(&row("Failed", &summary.failed_services.to_string()));
        self.body.append(&services);

        if !summary.repository_names.is_empty() {
            let repos = adw::PreferencesGroup::builder()
                .title(format!("Repositories ({})", summary.repository_names.len()))
                .build();
            for name in &summary.repository_names {
                repos.add(
                    &adw::ActionRow::builder()
                        .use_markup(false)
                        .title(name)
                        .build(),
                );
            }
            self.body.append(&repos);
        }
    }
}

fn row(title: &str, value: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .subtitle(value)
        .build()
}

fn spinner() -> gtk::Spinner {
    gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .margin_top(48)
        .build()
}
