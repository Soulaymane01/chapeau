use adw::prelude::*;
use chapeau::services::overview::Overview;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// The My System sidebar: sections of intentional resources.
pub struct Sidebar {
    pub list: gtk::ListBox,
    pub scan_button: gtk::Button,
    /// Native id for each list row position (`None` for section headers).
    rows: Rc<RefCell<Vec<Option<String>>>>,
}

impl Sidebar {
    pub fn new() -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .build();

        let scan_button = gtk::Button::builder()
            .label("Scan")
            .tooltip_text("Scan the system and update Chapeau's state")
            .build();

        Self {
            list,
            scan_button,
            rows: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// The native id of the resource at a list row index, if it is a
    /// resource row rather than a section header.
    pub fn native_id_at(&self, index: i32) -> Option<String> {
        self.rows.borrow().get(index as usize).cloned().flatten()
    }

    /// Rebuild the sidebar from an overview.
    pub fn populate(&self, view: &Overview) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.rows.borrow_mut().clear();

        if view.root_count == 0 {
            let empty = gtk::Label::builder()
                .label("No intentional resources yet")
                .wrap(true)
                .margin_top(24)
                .margin_bottom(24)
                .margin_start(12)
                .margin_end(12)
                .css_classes(["dim-label"])
                .build();
            self.list.append(&empty);
            self.rows.borrow_mut().push(None);
            return;
        }

        for section in &view.sections {
            let header = gtk::Label::builder()
                .label(format!("{} ({})", section.title, section.entries.len()))
                .halign(gtk::Align::Start)
                .margin_top(12)
                .margin_bottom(6)
                .margin_start(12)
                .margin_end(12)
                .css_classes(["dim-label"])
                .build();
            self.list.append(&header);
            self.rows.borrow_mut().push(None);

            for entry in &section.entries {
                let row = adw::ActionRow::builder()
                    .title(entry.label())
                    .subtitle(entry.tag())
                    .activatable(true)
                    .build();
                self.list.append(&row);
                self.rows
                    .borrow_mut()
                    .push(Some(entry.resource.native_id.clone()));
            }
        }
    }
}
