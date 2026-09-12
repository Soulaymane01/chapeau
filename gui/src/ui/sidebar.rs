use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

/// Sidebar navigation.
///
/// The sidebar only holds navigation categories; the actual content (roots,
/// packages, services, ...) lives in the main view. Rows activate the
/// application actions registered by `main`.
pub struct Sidebar {
    pub list: gtk::ListBox,
    pub scan_button: gtk::Button,
}

impl Sidebar {
    pub fn new(app: &adw::Application) -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .build();

        let scan_button = gtk::Button::builder()
            .label("Scan")
            .tooltip_text("Scan the system and update Chapeau's state")
            .build();

        list.append(&nav_row(app, "My System", "home", true));

        let system = adw::ExpanderRow::builder()
            .title("System")
            .subtitle("Health, services, organization, cleanup")
            .build();
        for (action, title) in [
            ("status", "Status"),
            ("drift", "Drift"),
            ("services", "Services"),
            ("domains", "Domains"),
            ("orphaned", "Orphaned"),
            ("unused", "Unused"),
        ] {
            system.add_row(&nav_row(app, title, action, false));
        }
        list.append(&system);
        system.set_expanded(true);

        let explore = adw::ExpanderRow::builder()
            .title("Explore")
            .subtitle("Everything Chapeau tracks")
            .build();
        for (action, title) in [
            ("explore-packages", "Packages"),
            ("explore-services", "Services"),
            ("explore-flatpaks", "Flatpaks"),
            ("explore-repositories", "Repositories"),
        ] {
            explore.add_row(&nav_row(app, title, action, false));
        }
        list.append(&explore);

        list.append(&nav_row(app, "About Chapeau", "about", false));

        Self { list, scan_button }
    }
}

/// A sidebar row that activates an application action.
fn nav_row(app: &adw::Application, title: &str, action: &str, home: bool) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .activatable(true)
        .build();
    if home {
        row.add_prefix(&gtk::Image::from_icon_name("go-home-symbolic"));
    }
    let app = app.clone();
    let action = action.to_string();
    row.connect_activated(move |_| {
        app.activate_action(&action, None);
    });
    row
}
