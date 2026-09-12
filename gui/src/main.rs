mod service;
mod ui;

use adw::prelude::*;
use chapeau::core::planner::RemovalPlan;
use chapeau::services::analysis::AnalysisKind;
use chapeau::services::explore::ExploreKind;
use chapeau::services::overview::Overview;
use chapeau::services::scan::ScanEvent;
use chapeau::storage::Database;
use gtk4 as gtk;
use gtk4::gio;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

use service::{Request, Response, Worker};

/// Explore menu entries: (action id, label, kind).
const EXPLORE_KINDS: [(&str, &str, ExploreKind); 4] = [
    ("explore-packages", "Packages", ExploreKind::Packages),
    ("explore-services", "Services", ExploreKind::Services),
    ("explore-flatpaks", "Flatpaks", ExploreKind::Flatpaks),
    (
        "explore-repositories",
        "Repositories",
        ExploreKind::Repositories,
    ),
];

fn main() -> anyhow::Result<()> {
    let app = adw::Application::builder()
        .application_id("org.chapeau.Chapeau")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    // `chapeau-gui <resource>` opens straight to that resource's detail.
    let initial = Rc::new(RefCell::new(None::<String>));
    let initial_cmdline = initial.clone();
    app.connect_command_line(move |app, cmdline| {
        if let Some(argument) = cmdline.arguments().get(1) {
            *initial_cmdline.borrow_mut() = Some(argument.to_string_lossy().into_owned());
        }
        app.activate();
        gtk::glib::ExitCode::SUCCESS
    });

    let initial_activate = initial.clone();
    app.connect_activate(move |app| {
        let resource = initial_activate.borrow_mut().take();
        build_ui(app, resource);
    });

    app.run();
    Ok(())
}

fn build_ui(app: &adw::Application, initial_resource: Option<String>) {
    gtk::glib::set_application_name("Chapeau");
    gtk::Window::set_default_icon_name("org.chapeau.Chapeau");

    let worker = Worker::spawn(Database::default_path());

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Chapeau")
        .default_width(1100)
        .default_height(750)
        .build();

    let sidebar = Rc::new(ui::sidebar::Sidebar::new());
    let nav = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    let graph_page = Rc::new(ui::graph::GraphPage::new());

    let detail_page = Rc::new(ui::detail::DetailPage::new(
        &window,
        {
            let worker = worker.clone();
            move |resource: String, domain: String, owns: bool| {
                worker.request(Request::DomainAddResource {
                    domain,
                    resource,
                    owns,
                    reason: None,
                });
            }
        },
        {
            let worker = worker.clone();
            move |resource: String, domain: String| {
                worker.request(Request::DomainRemoveResource { domain, resource });
            }
        },
        {
            let worker = worker.clone();
            let sidebar = sidebar.clone();
            move |resource: String| {
                sidebar.list.set_sensitive(false);
                worker.request(Request::RemovalPlan(resource));
            }
        },
        {
            let worker = worker.clone();
            let sidebar = sidebar.clone();
            let graph_page = graph_page.clone();
            let nav = nav.clone();
            move |native_id: String| {
                graph_page.show_loading(&native_id);
                nav.push(&graph_page.page);
                sidebar.list.set_sensitive(false);
                worker.request(Request::Graph(native_id));
            }
        },
    ));

    // Explore page (one reusable page; populated per kind).
    let explore_page = {
        let sidebar = sidebar.clone();
        let detail_page = detail_page.clone();
        let nav = nav.clone();
        let worker = worker.clone();
        Rc::new(ui::explore::ExplorePage::new(move |native_id| {
            open_detail(&sidebar, &detail_page, &nav, &worker, native_id);
        }))
    };

    // Set when a scan is started from the Drift page, so the report refreshes.
    let refresh_drift = Rc::new(RefCell::new(false));

    let status_page = {
        let worker = worker.clone();
        Rc::new(ui::status::StatusPage::new(move || {
            worker.request(Request::Status);
        }))
    };

    let drift_page = {
        let worker = worker.clone();
        let refresh_drift = refresh_drift.clone();
        let sidebar = sidebar.clone();
        Rc::new(ui::drift::DriftPage::new(move || {
            *refresh_drift.borrow_mut() = true;
            sidebar.scan_button.set_sensitive(false);
            sidebar.scan_button.set_label("Scanning…");
            sidebar.list.set_sensitive(false);
            worker.request(Request::Scan);
        }))
    };

    let domains_page = Rc::new(ui::domains::DomainsPage::new(
        &window,
        {
            let worker = worker.clone();
            move |name: String, description: Option<String>| {
                worker.request(Request::DomainCreate { name, description });
            }
        },
        {
            let worker = worker.clone();
            move |name: String| {
                worker.request(Request::DomainDelete { name });
            }
        },
        {
            let worker = worker.clone();
            move |domain: String, resource: String| {
                worker.request(Request::DomainRemoveResource { domain, resource });
            }
        },
    ));

    let analysis_page = {
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        Rc::new(ui::analysis::AnalysisPage::new(move |kind| {
            sidebar.list.set_sensitive(false);
            worker.request(Request::Analysis(kind));
        }))
    };

    // Home page shown when no resource is selected.
    let home_status = adw::StatusPage::builder()
        .icon_name("system-software-install-symbolic")
        .title("My System")
        .description("Loading…")
        .build();
    let home_page = adw::NavigationPage::builder()
        .title("My System")
        .child(&home_status)
        .build();
    nav.add(&home_page);

    // Layout: sidebar | content navigation.
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&sidebar.list)
        .vexpand(true)
        .build();
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.pack_end(&sidebar.scan_button);
    sidebar_header.pack_start(&build_menu(
        app,
        &explore_page,
        &status_page,
        &drift_page,
        &domains_page,
        &analysis_page,
        &nav,
        &worker,
        &sidebar,
    ));
    let sidebar_toolbar = adw::ToolbarView::builder().content(&scrolled).build();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    let sidebar_page = adw::NavigationPage::builder()
        .title("Chapeau")
        .child(&sidebar_toolbar)
        .build();
    let content_page = adw::NavigationPage::builder()
        .title("Chapeau")
        .child(&nav)
        .build();
    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .build();
    toasts.set_child(Some(&split));
    window.set_content(Some(&toasts));

    // Selecting a resource opens its detail page.
    {
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        let list = sidebar.list.clone();
        let detail_page = detail_page.clone();
        let nav = nav.clone();
        list.connect_row_activated(move |_, row| {
            let Some(native_id) = sidebar.native_id_at(row.index()) else {
                return;
            };
            open_detail(&sidebar, &detail_page, &nav, &worker, native_id);
        });
    }

    // Scan button.
    {
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        let scan_button = sidebar.scan_button.clone();
        scan_button.connect_clicked(move |button| {
            button.set_sensitive(false);
            button.set_label("Scanning…");
            sidebar.list.set_sensitive(false);
            worker.request(Request::Scan);
        });
    }

    // Worker responses are delivered on the main context.
    {
        let worker = worker.clone();
        let window = window.clone();
        let sidebar = sidebar.clone();
        let detail_page = detail_page.clone();
        let explore_page = explore_page.clone();
        let status_page = status_page.clone();
        let drift_page = drift_page.clone();
        let domains_page = domains_page.clone();
        let analysis_page = analysis_page.clone();
        let graph_page = graph_page.clone();
        let refresh_drift = refresh_drift.clone();
        let nav = nav.clone();
        let home_status = home_status.clone();
        let toasts = toasts.clone();

        let responses = worker.responses();
        gtk::glib::spawn_future_local(async move {
            while let Ok(response) = responses.recv().await {
                match response {
                    Response::Overview(Ok(view)) => {
                        sidebar.populate(&view);
                        update_home(&home_status, &view);
                    }
                    Response::Overview(Err(err)) => ui::present_error(&window, &err),
                    Response::Detail(Ok(detail)) => {
                        detail_page.populate(&detail);
                        sidebar.list.set_sensitive(true);
                    }
                    Response::Detail(Err(err)) => {
                        nav.pop();
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::Explore(Ok(view)) => {
                        explore_page.populate(&view);
                        sidebar.list.set_sensitive(true);
                    }
                    Response::Explore(Err(err)) => {
                        nav.pop();
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::Status(Ok(summary)) => {
                        status_page.populate(&summary);
                        sidebar.list.set_sensitive(true);
                    }
                    Response::Status(Err(err)) => {
                        nav.pop();
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::Drift(Ok(report)) => {
                        drift_page.populate(&report);
                        sidebar.list.set_sensitive(true);
                    }
                    Response::Drift(Err(err)) => {
                        nav.pop();
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::Domains(Ok(summaries)) => {
                        domains_page.populate(summaries.as_slice());
                        sidebar.list.set_sensitive(true);
                    }
                    Response::Domains(Err(err)) => {
                        nav.pop();
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::Analysis(kind, Ok(entries)) => {
                        analysis_page.populate(kind, entries.as_slice());
                        sidebar.list.set_sensitive(true);
                    }
                    Response::Analysis(_, Err(err)) => {
                        nav.pop();
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::DomainChanged(result) => match result {
                        Ok(()) => {
                            toasts.add_toast(adw::Toast::new("Domains updated"));
                            // Domain changes affect grouping, memberships and
                            // whatever detail page is showing.
                            worker.request(Request::Overview);
                            worker.request(Request::Domains);
                            if let Some(native_id) = detail_page.current_native_id() {
                                worker.request(Request::Detail(native_id));
                            }
                        }
                        Err(err) => ui::present_error(&window, &err),
                    },
                    Response::RemovalPlan(Ok(plan)) => {
                        sidebar.list.set_sensitive(true);
                        confirm_removal(&window, &plan, &worker);
                    }
                    Response::RemovalPlan(Err(err)) => {
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::RemovalDone(Ok(outcome)) => {
                        sidebar.list.set_sensitive(true);
                        if outcome.command_succeeded {
                            let message = match &outcome.reconcile {
                                Some(Ok(summary)) => format!(
                                    "Removed from Fedora ({} stale records reconciled)",
                                    summary.stale_packages_removed
                                ),
                                Some(Err(_)) => {
                                    "Removed from Fedora — run Scan to reconcile Chapeau"
                                        .to_string()
                                }
                                None => "Removed from Fedora".to_string(),
                            };
                            toasts.add_toast(adw::Toast::new(&message));
                            nav.pop();
                            worker.request(Request::Overview);
                        } else {
                            ui::present_error(
                                &window,
                                &format!("Removal failed.\n\n{}", outcome.stderr),
                            );
                        }
                    }
                    Response::RemovalDone(Err(err)) => {
                        sidebar.list.set_sensitive(true);
                        ui::present_error(&window, &err);
                    }
                    Response::Graph(Ok(bytes)) => {
                        sidebar.list.set_sensitive(true);
                        if let Err(err) = graph_page.show_png(bytes) {
                            nav.pop();
                            ui::present_error(&window, &err);
                        }
                    }
                    Response::Graph(Err(err)) => {
                        sidebar.list.set_sensitive(true);
                        nav.pop();
                        ui::present_error(&window, &err);
                    }
                    Response::ScanProgress(event) => {
                        home_status.set_description(Some(&scan_progress_text(&event)));
                    }
                    Response::ScanDone(result) => {
                        sidebar.scan_button.set_sensitive(true);
                        sidebar.scan_button.set_label("Scan");
                        sidebar.list.set_sensitive(true);
                        match result {
                            Ok(outcome) => {
                                let summary = if outcome.drift.is_empty() {
                                    "Scan complete".to_string()
                                } else {
                                    format!("Scan complete: {}", outcome.drift.summary())
                                };
                                toasts.add_toast(adw::Toast::new(&summary));
                            }
                            Err(err) => ui::present_error(&window, &err),
                        }
                        // Refresh the model regardless of the outcome.
                        worker.request(Request::Overview);
                        if *refresh_drift.borrow() {
                            *refresh_drift.borrow_mut() = false;
                            worker.request(Request::Drift);
                        }
                    }
                }
            }
        });
    }

    window.present();
    worker.request(Request::Overview);

    // `chapeau-gui <resource>` opens straight to that resource's detail.
    if let Some(native_id) = initial_resource {
        open_detail(&sidebar, &detail_page, &nav, &worker, native_id);
    }
}

fn open_detail(
    sidebar: &ui::sidebar::Sidebar,
    detail_page: &ui::detail::DetailPage,
    nav: &adw::NavigationView,
    worker: &Worker,
    native_id: String,
) {
    detail_page.set_loading(&native_id);
    nav.push(&detail_page.page);
    sidebar.list.set_sensitive(false);
    worker.request(Request::Detail(native_id));
}

/// Header menu offering the Explore views.
#[allow(clippy::too_many_arguments)]
fn build_menu(
    app: &adw::Application,
    explore_page: &Rc<ui::explore::ExplorePage>,
    status_page: &Rc<ui::status::StatusPage>,
    drift_page: &Rc<ui::drift::DriftPage>,
    domains_page: &Rc<ui::domains::DomainsPage>,
    analysis_page: &Rc<ui::analysis::AnalysisPage>,
    nav: &adw::NavigationView,
    worker: &Worker,
    sidebar: &Rc<ui::sidebar::Sidebar>,
) -> gtk::MenuButton {
    let menu = gio::Menu::new();

    let system = gio::Menu::new();
    system.append(Some("Status"), Some("app.status"));
    system.append(Some("Drift"), Some("app.drift"));
    system.append(Some("Domains"), Some("app.domains"));
    system.append(Some("Orphaned"), Some("app.orphaned"));
    system.append(Some("Unused"), Some("app.unused"));
    menu.append_section(Some("System"), &system);

    let explore = gio::Menu::new();
    for (id, label, _) in EXPLORE_KINDS {
        explore.append(Some(label), Some(&format!("app.{id}")));
    }
    menu.append_section(Some("Explore"), &explore);

    // Status action
    {
        let action = gio::SimpleAction::new("status", None);
        let status_page = status_page.clone();
        let nav = nav.clone();
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        action.connect_activate(move |_, _| {
            status_page.set_loading();
            nav.push(&status_page.page);
            sidebar.list.set_sensitive(false);
            worker.request(Request::Status);
        });
        app.add_action(&action);
    }

    // Drift action
    {
        let action = gio::SimpleAction::new("drift", None);
        let drift_page = drift_page.clone();
        let nav = nav.clone();
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        action.connect_activate(move |_, _| {
            drift_page.set_loading();
            nav.push(&drift_page.page);
            sidebar.list.set_sensitive(false);
            worker.request(Request::Drift);
        });
        app.add_action(&action);
    }

    // Domains action
    {
        let action = gio::SimpleAction::new("domains", None);
        let domains_page = domains_page.clone();
        let nav = nav.clone();
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        action.connect_activate(move |_, _| {
            domains_page.set_loading();
            nav.push(&domains_page.page);
            sidebar.list.set_sensitive(false);
            worker.request(Request::Domains);
        });
        app.add_action(&action);
    }

    // Cleanup analysis actions
    for (id, kind) in [
        ("orphaned", AnalysisKind::Orphaned),
        ("unused", AnalysisKind::Unused),
    ] {
        let action = gio::SimpleAction::new(id, None);
        let analysis_page = analysis_page.clone();
        let nav = nav.clone();
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        action.connect_activate(move |_, _| {
            analysis_page.set_loading(kind);
            nav.push(&analysis_page.page);
            sidebar.list.set_sensitive(false);
            worker.request(Request::Analysis(kind));
        });
        app.add_action(&action);
    }

    for (id, _, kind) in EXPLORE_KINDS {
        let action = gio::SimpleAction::new(id, None);
        let explore_page = explore_page.clone();
        let nav = nav.clone();
        let worker = worker.clone();
        let sidebar = sidebar.clone();
        action.connect_activate(move |_, _| {
            explore_page.set_loading(kind.title());
            nav.push(&explore_page.page);
            sidebar.list.set_sensitive(false);
            worker.request(Request::Explore(kind));
        });
        app.add_action(&action);
    }

    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("System views")
        .build()
}

/// Show the impact plan and, on confirmation, execute the removal with pkexec.
fn confirm_removal(window: &adw::ApplicationWindow, plan: &RemovalPlan, worker: &Worker) {
    let label = gtk::Label::builder()
        .label(plan.format())
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .css_classes(["monospace"])
        .build();
    let content = gtk::ScrolledWindow::builder()
        .child(&label)
        .min_content_height(120)
        .max_content_height(320)
        .build();

    let dialog = adw::AlertDialog::builder()
        .heading(format!("Remove {}?", plan.resource.native_id))
        .body("This runs the native package manager. Chapeau's own state is only updated after the system change succeeds.")
        .extra_child(&content)
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("remove", "Remove");
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_close_response("cancel");

    let worker = worker.clone();
    let native_id = plan.resource.native_id.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "remove" {
            worker.request(Request::RemovalExecute(native_id.clone()));
        }
    });

    dialog.present(Some(window));
}

fn update_home(status: &adw::StatusPage, view: &Overview) {
    if view.resource_count == 0 {
        status.set_description(Some(
            "No system state yet. Click Scan to discover what is installed.",
        ));
        return;
    }

    let missing = if view.missing_count > 0 {
        format!(" · {} missing", view.missing_count)
    } else {
        String::new()
    };
    status.set_description(Some(&format!(
        "{} intentional resources{} · {} tracked resources · {} relationships\nSelect a resource from the sidebar, or use Scan to refresh.",
        view.root_count, missing, view.resource_count, view.relationship_count
    )));
}

fn scan_progress_text(event: &ScanEvent) -> String {
    match event {
        ScanEvent::Discovering { backend } => format!("Scanning: discovering {backend}…"),
        ScanEvent::Discovered { backend, resources } => format!("Discovered {resources} {backend}"),
        ScanEvent::Dependencies => {
            "Scanning package dependencies (this can take a while)…".to_string()
        }
        ScanEvent::DependenciesDone { count } => format!("Resolved {count} dependency edges"),
        ScanEvent::Committing => "Saving results…".to_string(),
    }
}
