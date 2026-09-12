use adw::prelude::*;
use chapeau::services::explore::{item_label, ExploreItem, ExploreView};
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;

type OpenCallback = Rc<dyn Fn(String)>;

/// Flatpak applications by default; everything (including runtimes) with the
/// "All Flatpaks" switch.
pub struct FlatpaksPage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
    all_switch: gtk::Switch,
    syncing: Rc<Cell<bool>>,
    show_all: Rc<Cell<bool>>,
    on_open: OpenCallback,
}

impl FlatpaksPage {
    pub fn new<F, G>(on_reload: F, on_open: G) -> Self
    where
        F: Fn(bool) + 'static,
        G: Fn(String) + 'static,
    {
        let all_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        let switch_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        switch_box.append(&gtk::Label::new(Some("All Flatpaks")));
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
            .title("Flatpaks")
            .tag("flatpaks")
            .child(&toolbar)
            .build();

        let show_all = Rc::new(Cell::new(false));
        let syncing = Rc::new(Cell::new(false));
        {
            let show_all = show_all.clone();
            let syncing = syncing.clone();
            let on_reload = Rc::new(on_reload);
            all_switch.connect_active_notify(move |switch| {
                if syncing.get() {
                    return;
                }
                let value = switch.is_active();
                show_all.set(value);
                on_reload(value);
            });
        }

        Self {
            page,
            body,
            all_switch,
            syncing,
            show_all,
            on_open: Rc::new(on_open),
        }
    }

    pub fn set_loading(&self, show_all: bool) {
        self.show_all.set(show_all);
        self.page.set_title(if show_all {
            "Flatpaks (all)"
        } else {
            "Flatpaks"
        });
        self.syncing.set(true);
        self.all_switch.set_active(show_all);
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

    pub fn populate(&self, view: &ExploreView) {
        self.clear();

        let items: Vec<&ExploreItem> = if self.show_all.get() {
            view.items.iter().collect()
        } else {
            view.items.iter().filter(|item| item.is_root).collect()
        };

        if items.is_empty() {
            let status = adw::StatusPage::builder()
                .icon_name("system-software-install-symbolic")
                .title("No intentional Flatpak applications")
                .description(if self.show_all.get() {
                    "No Flatpaks in the model. Run a scan first."
                } else {
                    "Flip \"All Flatpaks\" to see runtimes and every installed application."
                })
                .margin_top(48)
                .build();
            self.body.append(&status);
            return;
        }

        if self.show_all.get() {
            let applications: Vec<&ExploreItem> = items
                .iter()
                .copied()
                .filter(|item| !is_runtime(item))
                .collect();
            let runtimes: Vec<&ExploreItem> = items
                .iter()
                .copied()
                .filter(|item| is_runtime(item))
                .collect();
            self.append_group("Applications", &applications);
            self.append_group("Runtimes", &runtimes);
        } else {
            self.append_group("Intentional Flatpaks", &items);
        }
    }

    fn append_group(&self, title: &str, items: &[&ExploreItem]) {
        if items.is_empty() {
            return;
        }

        let group = adw::PreferencesGroup::builder()
            .title(format!("{} ({})", title, items.len()))
            .build();

        for item in items {
            let mut tags = Vec::new();
            if item.is_root {
                tags.push("root");
            }
            if item.missing {
                tags.push("missing");
            }

            let mut parts = Vec::new();
            if let Some(version) = &item.version {
                parts.push(version.clone());
            }
            if let Some(detail) = &item.detail {
                parts.push(detail.clone());
            }
            if let Some(source) = &item.source {
                parts.push(source.clone());
            }
            let details = parts.join(" · ");
            let subtitle = if tags.is_empty() {
                details
            } else {
                format!("[{}] {}", tags.join(", "), details)
            };

            let row = adw::ActionRow::builder()
                .use_markup(false)
                .title(item_label(item))
                .subtitle(subtitle)
                .activatable(true)
                .build();
            let on_open = self.on_open.clone();
            let native_id = item.resource.native_id.clone();
            row.connect_activated(move |_| on_open(native_id.clone()));
            group.add(&row);
        }

        self.body.append(&group);
    }
}

fn is_runtime(item: &ExploreItem) -> bool {
    item.detail
        .as_deref()
        .is_some_and(|detail| detail.starts_with("runtime"))
}
