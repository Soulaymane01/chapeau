use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

/// Displays a rendered Graphviz neighborhood.
pub struct GraphPage {
    pub page: adw::NavigationPage,
    body: gtk::Box,
}

impl GraphPage {
    pub fn new() -> Self {
        let header = adw::HeaderBar::new();
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&body)
            .vexpand(true)
            .hexpand(true)
            .build();
        let toolbar = adw::ToolbarView::builder().content(&scrolled).build();
        toolbar.add_top_bar(&header);
        let page = adw::NavigationPage::builder()
            .title("Graph")
            .tag("graph")
            .child(&toolbar)
            .build();

        Self { page, body }
    }

    pub fn show_loading(&self, title: &str) {
        self.page.set_title(title);
        self.clear();
        self.body
            .append(&gtk::Spinner::builder().spinning(true).build());
    }

    /// Show PNG bytes rendered by Graphviz.
    pub fn show_png(&self, bytes: Vec<u8>) -> Result<(), String> {
        let texture = gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_owned(bytes))
            .map_err(|err| err.to_string())?;
        self.clear();
        let picture = gtk::Picture::for_paintable(&texture);
        picture.set_can_shrink(true);
        self.body.append(&picture);
        Ok(())
    }

    fn clear(&self) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }
}
