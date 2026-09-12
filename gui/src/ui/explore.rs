use adw::prelude::*;
use chapeau::services::explore::{self, ExploreItem, ExploreView};
use gtk4 as gtk;
use gtk4::gio;
use gtk4::glib;
use gtk4::glib::subclass::prelude::*;
use gtk4::glib::Properties;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod imp {
    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::ExploreItemObject)]
    pub struct ExploreItemObject {
        #[property(get, set)]
        pub title: RefCell<String>,
        #[property(get, set)]
        pub subtitle: RefCell<String>,
        #[property(get, set)]
        pub search_text: RefCell<String>,
        #[property(get, set)]
        pub native_id: RefCell<String>,
        #[property(get, set)]
        pub is_root: Cell<bool>,
        #[property(get, set)]
        pub is_user: Cell<bool>,
        #[property(get, set)]
        pub missing: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExploreItemObject {
        const NAME: &'static str = "ChapeauExploreItem";
        type Type = super::ExploreItemObject;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ExploreItemObject {}
}

glib::wrapper! {
    pub struct ExploreItemObject(ObjectSubclass<imp::ExploreItemObject>);
}

impl ExploreItemObject {
    fn from_item(item: &ExploreItem) -> Self {
        let title = explore::item_label(item);
        let subtitle = item_subtitle(item);
        let object: Self = glib::Object::builder().build();
        object.set_search_text(format!("{title} {subtitle} {}", item.resource.native_id));
        object.set_native_id(item.resource.native_id.clone());
        object.set_title(title);
        object.set_subtitle(subtitle);
        object.set_is_root(item.is_root);
        object.set_is_user(item.is_user);
        object.set_missing(item.missing);
        object
    }
}

/// Browse/search page for one resource kind.
pub struct ExplorePage {
    pub page: adw::NavigationPage,
    header: adw::HeaderBar,
    search: gtk::SearchEntry,
    model: gio::ListStore,
}

impl ExplorePage {
    pub fn new<F: Fn(String) + 'static>(on_activate: F) -> Self {
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search")
            .width_request(280)
            .build();
        let all_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        let switch_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        switch_box.append(&gtk::Label::new(Some("All")));
        switch_box.append(&all_switch);
        switch_box.set_tooltip_text(Some("Show every tracked resource"));
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&search));
        header.pack_start(&switch_box);

        let model = gio::ListStore::new::<ExploreItemObject>();

        let expression = gtk::PropertyExpression::new(
            ExploreItemObject::static_type(),
            None::<&gtk::Expression>,
            "search-text",
        );
        let filter = gtk::StringFilter::new(Some(expression.upcast_ref()));
        filter.set_match_mode(gtk::StringFilterMatchMode::Substring);

        // "All" switch: off keeps only resources a normal user would consider
        // intentional; on shows everything tracked.
        let user_only = Rc::new(Cell::new(true));
        let custom = {
            let user_only = user_only.clone();
            gtk::CustomFilter::new(move |object| {
                let Some(item) = object.downcast_ref::<ExploreItemObject>() else {
                    return true;
                };
                !user_only.get() || item.is_user()
            })
        };
        let every = gtk::EveryFilter::new();
        every.append(filter.clone());
        every.append(custom.clone());

        let filter_model = gtk::FilterListModel::new(Some(model.clone()), Some(every));
        let selection = gtk::NoSelection::new(Some(filter_model));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let Some(list_item) = list_item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            list_item.set_child(Some(&row));
        });
        factory.connect_bind(|_, list_item| {
            let Some(list_item) = list_item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(item) = list_item.item().and_downcast::<ExploreItemObject>() else {
                return;
            };
            let Some(row) = list_item.child().and_downcast::<adw::ActionRow>() else {
                return;
            };
            row.set_title(&item.title());

            let mut tags = Vec::new();
            if item.is_root() {
                tags.push("root");
            }
            if item.missing() {
                tags.push("missing");
            }
            let subtitle = item.subtitle();
            if tags.is_empty() {
                row.set_subtitle(&subtitle);
            } else {
                row.set_subtitle(&format!("[{}] {}", tags.join(", "), subtitle));
            }
        });

        let list_view = gtk::ListView::builder()
            .model(&selection)
            .factory(&factory)
            .build();
        list_view.connect_activate(move |view, position| {
            let Some(item) = view
                .model()
                .and_then(|model| model.item(position))
                .and_downcast::<ExploreItemObject>()
            else {
                return;
            };
            on_activate(item.native_id());
        });

        let scrolled = gtk::ScrolledWindow::builder()
            .child(&list_view)
            .vexpand(true)
            .build();
        let toolbar = adw::ToolbarView::builder().content(&scrolled).build();
        toolbar.add_top_bar(&header);
        let page = adw::NavigationPage::builder()
            .title("Explore")
            .tag("explore")
            .child(&toolbar)
            .build();

        {
            let filter = filter.clone();
            search.connect_search_changed(move |entry| {
                let text = entry.text();
                filter.set_search(Some(text.as_str()).filter(|value| !value.is_empty()));
            });
        }
        {
            let user_only = user_only.clone();
            let custom = custom.clone();
            all_switch.connect_active_notify(move |switch| {
                user_only.set(!switch.is_active());
                custom.changed(gtk::FilterChange::Different);
            });
        }

        Self {
            page,
            header,
            search,
            model,
        }
    }

    /// Clear the list while new data is loading.
    pub fn set_loading(&self, title: &str) {
        self.page.set_title(title);
        self.model.remove_all();
    }

    /// Replace the contents with a fresh explore view.
    pub fn populate(&self, view: &ExploreView) {
        self.page
            .set_title(&format!("{} ({})", view.kind.title(), view.items.len()));
        self.header.set_title_widget(Some(&self.search));
        self.model.remove_all();
        for item in &view.items {
            self.model.append(&ExploreItemObject::from_item(item));
        }
    }
}

fn item_subtitle(item: &ExploreItem) -> String {
    let mut parts = Vec::new();

    if let Some(version) = &item.version {
        if item.missing {
            parts.push(format!("{version} (last seen)"));
        } else {
            parts.push(version.clone());
        }
    }

    if item.resource.resource_type == chapeau::core::ResourceType::Service {
        parts.push(service_state(item));
    }

    if let Some(detail) = &item.detail {
        parts.push(detail.clone());
    }
    if let Some(count) = item.package_count {
        parts.push(format!("{count} packages"));
    }
    if let Some(source) = &item.source {
        parts.push(source.clone());
    }

    if parts.is_empty() {
        "no recorded state".to_string()
    } else {
        parts.join(" · ")
    }
}

fn service_state(item: &ExploreItem) -> String {
    let mut parts = Vec::new();
    match item.active {
        Some(true) => parts.push("active"),
        Some(false) => parts.push("inactive"),
        None => {}
    }
    match item.enabled {
        Some(true) => parts.push("enabled"),
        Some(false) => parts.push("disabled"),
        None => {}
    }
    if item.failed == Some(true) {
        parts.push("failed");
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(", ")
    }
}
