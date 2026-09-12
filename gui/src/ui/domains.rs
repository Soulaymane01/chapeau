use adw::prelude::*;
use chapeau::services::domains::DomainSummary;
use gtk4 as gtk;
use libadwaita as adw;
use std::rc::Rc;

type CreateCallback = Rc<dyn Fn(String, Option<String>)>;
type DeleteCallback = Rc<dyn Fn(String)>;
type RemoveResourceCallback = Rc<dyn Fn(String, String)>;

/// Domain management: list, create, delete, and edit memberships.
pub struct DomainsPage {
    pub page: adw::NavigationPage,
    window: adw::ApplicationWindow,
    body: gtk::Box,
    on_delete: DeleteCallback,
    on_remove_resource: RemoveResourceCallback,
}

impl DomainsPage {
    pub fn new<FC, FD, FR>(
        window: &adw::ApplicationWindow,
        on_create: FC,
        on_delete: FD,
        on_remove_resource: FR,
    ) -> Self
    where
        FC: Fn(String, Option<String>) + 'static,
        FD: Fn(String) + 'static,
        FR: Fn(String, String) + 'static,
    {
        let header = adw::HeaderBar::new();
        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Create a domain")
            .build();
        header.pack_end(&add);

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
            .title("Domains")
            .tag("domains")
            .child(&toolbar)
            .build();

        let on_create: CreateCallback = Rc::new(on_create);
        let on_delete: DeleteCallback = Rc::new(on_delete);
        let on_remove_resource: RemoveResourceCallback = Rc::new(on_remove_resource);

        {
            let window = window.clone();
            let on_create = on_create.clone();
            add.connect_clicked(move |_| create_dialog(&window, &on_create));
        }

        Self {
            page,
            window: window.clone(),
            body,
            on_delete,
            on_remove_resource,
        }
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

    pub fn populate(&self, summaries: &[DomainSummary]) {
        self.clear();

        if summaries.is_empty() {
            let status = adw::StatusPage::builder()
                .icon_name("folder-symbolic")
                .title("No domains yet")
                .description(
                    "Domains are your own organization. Create one to group related resources — for example Development, Databases or AI.",
                )
                .margin_top(48)
                .build();
            self.body.append(&status);
            return;
        }

        let group = adw::PreferencesGroup::builder().title("Domains").build();
        for summary in summaries {
            let subtitle = summary
                .domain
                .description
                .clone()
                .unwrap_or_else(|| format!("{} resources", summary.resources.len()));
            let expander = adw::ExpanderRow::builder()
                .use_markup(false)
                .title(&summary.domain.name)
                .subtitle(subtitle)
                .build();

            let delete = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Delete domain")
                .css_classes(["flat"])
                .build();
            {
                let window = self.window.clone();
                let name = summary.domain.name.clone();
                let on_delete = self.on_delete.clone();
                delete.connect_clicked(move |_| confirm_delete(&window, &name, &on_delete));
            }
            expander.add_suffix(&delete);

            if summary.resources.is_empty() {
                expander.add_row(
                    &adw::ActionRow::builder()
                        .use_markup(false)
                        .title("No resources yet")
                        .build(),
                );
            } else {
                for (resource, relationship) in &summary.resources {
                    let label = resource
                        .display_name
                        .clone()
                        .unwrap_or_else(|| resource.native_id.clone());
                    let row = adw::ActionRow::builder()
                        .use_markup(false)
                        .title(&label)
                        .subtitle(relationship.to_string())
                        .build();

                    let remove = gtk::Button::builder()
                        .icon_name("list-remove-symbolic")
                        .tooltip_text("Remove from domain")
                        .css_classes(["flat"])
                        .build();
                    {
                        let domain = summary.domain.name.clone();
                        let native_id = resource.native_id.clone();
                        let on_remove = self.on_remove_resource.clone();
                        remove
                            .connect_clicked(move |_| on_remove(domain.clone(), native_id.clone()));
                    }
                    row.add_suffix(&remove);
                    expander.add_row(&row);
                }
            }

            group.add(&expander);
        }
        self.body.append(&group);
    }
}

fn create_dialog(window: &adw::ApplicationWindow, on_create: &CreateCallback) {
    let name = gtk::Entry::builder()
        .placeholder_text("Domain name")
        .activates_default(true)
        .build();
    let description = gtk::Entry::builder()
        .placeholder_text("Description (optional)")
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();
    content.append(&name);
    content.append(&description);

    let dialog = adw::AlertDialog::builder()
        .heading("New domain")
        .body("Group related resources under a name. Domains are yours to define.")
        .extra_child(&content)
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("create", "Create");
    dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("create"));
    dialog.set_close_response("cancel");

    let on_create = on_create.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "create" {
            return;
        }
        let domain_name = name.text().trim().to_string();
        if domain_name.is_empty() {
            return;
        }
        let description = description.text().trim().to_string();
        let description = if description.is_empty() {
            None
        } else {
            Some(description)
        };
        on_create(domain_name, description);
    });

    dialog.present(Some(window));
}

fn confirm_delete(window: &adw::ApplicationWindow, name: &str, on_delete: &DeleteCallback) {
    let dialog = adw::AlertDialog::builder()
        .heading(format!("Delete domain '{name}'?"))
        .body("Memberships are removed. The resources themselves are never touched.")
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("delete", "Delete");
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_close_response("cancel");

    let on_delete = on_delete.clone();
    let name = name.to_string();
    dialog.connect_response(None, move |_, response| {
        if response == "delete" {
            on_delete(name.clone());
        }
    });

    dialog.present(Some(window));
}

fn spinner() -> gtk::Spinner {
    gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .margin_top(48)
        .build()
}
