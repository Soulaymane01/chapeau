use adw::prelude::*;
use chapeau::services::units::{ServiceAction, ServiceGroup, ServiceState};
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;

type ActionCallback = Rc<dyn Fn(String, ServiceAction)>;

/// systemd services grouped by state, with start/stop actions.
pub struct ServicesPage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
    user_only: Rc<Cell<bool>>,
    all_switch: gtk::Switch,
    syncing: Rc<Cell<bool>>,
    on_action: ActionCallback,
}

impl ServicesPage {
    pub fn new<F, G>(on_reload: F, on_action: G) -> Self
    where
        F: Fn(bool) + 'static,
        G: Fn(String, ServiceAction) + 'static,
    {
        let all_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        let switch_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        switch_box.append(&gtk::Label::new(Some("All services")));
        switch_box.append(&all_switch);

        let header = adw::HeaderBar::new();
        header.pack_start(&switch_box);

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
            .title("Services")
            .child(&toolbar)
            .build();

        let user_only = Rc::new(Cell::new(true));
        let syncing = Rc::new(Cell::new(false));
        {
            let user_only = user_only.clone();
            let syncing = syncing.clone();
            let on_reload = Rc::new(on_reload);
            all_switch.connect_active_notify(move |switch| {
                if syncing.get() {
                    return;
                }
                // "All services" on => user_only off.
                let value = !switch.is_active();
                user_only.set(value);
                on_reload(value);
            });
        }

        Self {
            page,
            body,
            user_only,
            all_switch,
            syncing,
            on_action: Rc::new(on_action),
        }
    }

    /// The current filter state (true = user services only).
    pub fn user_only(&self) -> bool {
        self.user_only.get()
    }

    pub fn set_loading(&self, user_only: bool) {
        self.user_only.set(user_only);
        self.page.set_title(if user_only {
            "Services"
        } else {
            "Services (all)"
        });
        self.syncing.set(true);
        self.all_switch.set_active(!user_only);
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

    pub fn populate(&self, groups: &[ServiceGroup]) {
        self.clear();

        if groups.is_empty() {
            let status = adw::StatusPage::builder()
                .icon_name("emblem-ok-symbolic")
                .title("No services to show")
                .description(if self.user_only.get() {
                    "No user services found. Flip \"All services\" to see every unit."
                } else {
                    "No services in the model. Run a scan first."
                })
                .margin_top(48)
                .build();
            self.body.append(&status);
            return;
        }

        self.body.append(
            &adw::Banner::builder()
                .title(
                    "Starting and stopping services asks for authorization (systemctl via polkit)",
                )
                .build(),
        );

        for group in groups {
            let preferences = adw::PreferencesGroup::builder()
                .title(format!("{} ({})", group.title, group.entries.len()))
                .build();

            for entry in &group.entries {
                let enabled = match entry.enabled {
                    Some(true) => "enabled",
                    Some(false) => "disabled",
                    None => "unknown",
                };
                let mut subtitle = enabled.to_string();
                if !entry.provided_by.is_empty() {
                    subtitle.push_str(&format!(" · provided by {}", entry.provided_by.join(", ")));
                }
                if !entry.is_user {
                    subtitle.push_str(" · system");
                }

                let row = adw::ActionRow::builder()
                    .title(&entry.resource.native_id)
                    .subtitle(subtitle)
                    .build();

                let (label, action) = match entry.state {
                    ServiceState::Running => ("Stop", ServiceAction::Stop),
                    ServiceState::Failed | ServiceState::Stopped => ("Start", ServiceAction::Start),
                };
                let button = gtk::Button::builder()
                    .label(label)
                    .valign(gtk::Align::Center)
                    .build();
                if action == ServiceAction::Start {
                    button.add_css_class("suggested-action");
                } else {
                    button.add_css_class("flat");
                }

                let on_action = self.on_action.clone();
                let unit = entry.resource.native_id.clone();
                button.connect_clicked(move |_| on_action(unit.clone(), action));
                row.add_suffix(&button);

                let (boot_label, boot_action, boot_tip) = match entry.enabled {
                    Some(true) => (
                        "Disable",
                        ServiceAction::Disable,
                        "Do not start this service at boot",
                    ),
                    _ => (
                        "Enable",
                        ServiceAction::Enable,
                        "Start this service at boot",
                    ),
                };
                let boot_button = gtk::Button::builder()
                    .label(boot_label)
                    .valign(gtk::Align::Center)
                    .tooltip_text(boot_tip)
                    .css_classes(["flat"])
                    .build();
                let on_action = self.on_action.clone();
                let unit = entry.resource.native_id.clone();
                boot_button.connect_clicked(move |_| on_action(unit.clone(), boot_action));
                row.add_suffix(&boot_button);

                preferences.add(&row);
            }

            self.body.append(&preferences);
        }
    }
}
