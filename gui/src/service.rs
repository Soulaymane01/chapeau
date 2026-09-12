use async_channel::{Receiver, Sender};
use chapeau::core::planner::RemovalPlan;
use chapeau::core::RelationshipType;
use chapeau::services::analysis::{self, AnalysisKind, ResourceAnalysis};
use chapeau::services::detail::ResourceDetail;
use chapeau::services::domains::{self, DomainSummary};
use chapeau::services::drift::DriftReport;
use chapeau::services::explore::{self, ExploreKind, ExploreView};
use chapeau::services::graph::{self, GraphOptions};
use chapeau::services::overview::Overview;
use chapeau::services::removal::{self, Privilege, RemovalOutcome};
use chapeau::services::roots as root_actions;
use chapeau::services::scan::{self, ScanEvent, ScanOutcome};
use chapeau::services::status::StatusSummary;
use chapeau::services::units::{self, ServiceAction, ServiceGroup};
use chapeau::services::{detail, drift, overview, status};
use chapeau::storage::{resources, Database};
use std::path::PathBuf;

/// Work sent to the database/scan worker thread.
pub enum Request {
    Overview,
    Detail(String),
    Explore(ExploreKind),
    Status,
    Drift,
    Domains,
    Analysis(AnalysisKind),
    Units {
        user_only: bool,
    },
    UnitControl {
        unit: String,
        action: ServiceAction,
    },
    ToggleHidden {
        resource: String,
        hidden: bool,
    },
    DomainCreate {
        name: String,
        description: Option<String>,
    },
    DomainDelete {
        name: String,
    },
    DomainAddResource {
        domain: String,
        resource: String,
        owns: bool,
        reason: Option<String>,
    },
    DomainRemoveResource {
        domain: String,
        resource: String,
    },
    RemovalPlan(String),
    RemovalExecute(String),
    Graph(String),
    Scan,
}

/// Results delivered back to the GTK main loop.
pub enum Response {
    Overview(Result<Overview, String>),
    Detail(Result<Box<ResourceDetail>, String>),
    Explore(Result<Box<ExploreView>, String>),
    Status(Result<Box<StatusSummary>, String>),
    Drift(Result<Box<DriftReport>, String>),
    Domains(Result<Vec<DomainSummary>, String>),
    Analysis(AnalysisKind, Result<Vec<ResourceAnalysis>, String>),
    Units(Result<Vec<ServiceGroup>, String>),
    UnitChanged(Result<(), String>),
    RootChanged(Result<(), String>),
    /// Result of a domain mutation; frontends refresh what they show.
    DomainChanged(Result<(), String>),
    /// The worker failed unexpectedly while handling a request.
    WorkerFailed(String),
    RemovalPlan(Result<Box<RemovalPlan>, String>),
    RemovalDone(Result<Box<RemovalOutcome>, String>),
    /// Rendered Graphviz neighborhood (PNG bytes).
    Graph(Result<Vec<u8>, String>),
    ScanProgress(ScanEvent),
    ScanDone(Result<ScanOutcome, String>),
}

/// Owns the database connection on a dedicated thread.
///
/// The GUI never touches SQLite on the main thread: every query is a request,
/// every result a response delivered through the main context.
#[derive(Clone)]
pub struct Worker {
    requests: Sender<Request>,
    responses: Receiver<Response>,
}

impl Worker {
    pub fn spawn(db_path: PathBuf) -> Self {
        let (requests_tx, requests_rx) = async_channel::unbounded::<Request>();
        let (responses_tx, responses_rx) = async_channel::unbounded::<Response>();

        std::thread::spawn(move || {
            let db = match Database::open(&db_path) {
                Ok(db) => db,
                Err(err) => {
                    let _ = responses_tx.send_blocking(Response::Overview(Err(format!(
                        "failed to open database: {err}"
                    ))));
                    return;
                }
            };

            while let Ok(request) = requests_rx.recv_blocking() {
                let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_request(&db, request, &responses_tx);
                }));
                if handled.is_err() {
                    let _ = responses_tx.send_blocking(Response::WorkerFailed(
                        "Chapeau hit an internal error while handling that action.".to_string(),
                    ));
                }
            }
        });

        Self {
            requests: requests_tx,
            responses: responses_rx,
        }
    }

    pub fn request(&self, request: Request) {
        let _ = self.requests.send_blocking(request);
    }

    pub fn responses(&self) -> Receiver<Response> {
        self.responses.clone()
    }
}

fn handle_request(db: &Database, request: Request, responses: &async_channel::Sender<Response>) {
    match request {
        Request::Overview => {
            let response = overview::build(db).map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Overview(response));
        }
        Request::Detail(name) => {
            let response = fetch_detail(db, &name).map(Box::new);
            let _ = responses.send_blocking(Response::Detail(response));
        }
        Request::Explore(kind) => {
            let response = explore::build(db, kind)
                .map(Box::new)
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Explore(response));
        }
        Request::Status => {
            let response = status::summary(db)
                .map(Box::new)
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Status(response));
        }
        Request::Drift => {
            let response = drift::detect(db)
                .map(Box::new)
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Drift(response));
        }
        Request::Domains => {
            let response = domains::list(db).map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Domains(response));
        }
        Request::Analysis(kind) => {
            let response = analysis::run(db, kind).map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Analysis(kind, response));
        }
        Request::Units { user_only } => {
            let response = units::list(db, user_only).map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Units(response));
        }
        Request::UnitControl { unit, action } => {
            let result = units::control(db, &unit, action, Privilege::Pkexec)
                .map_err(|err| err.to_string())
                .and_then(|outcome| {
                    if outcome.succeeded {
                        Ok(())
                    } else {
                        Err(format!(
                            "Could not {} {}: {}",
                            action.verb(),
                            unit,
                            outcome.stderr.lines().next().unwrap_or("unknown error")
                        ))
                    }
                });
            let _ = responses.send_blocking(Response::UnitChanged(result));
        }
        Request::ToggleHidden { resource, hidden } => {
            let result = if hidden {
                root_actions::hide(db, &resource).map(|_| ())
            } else {
                root_actions::unhide(db, &resource).map(|_| ())
            }
            .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::RootChanged(result));
        }
        Request::DomainCreate { name, description } => {
            let result = domains::create(db, &name, description.as_deref())
                .map(|_| ())
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::DomainChanged(result));
        }
        Request::DomainDelete { name } => {
            let result = domains::delete(db, &name)
                .map(|_| ())
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::DomainChanged(result));
        }
        Request::DomainAddResource {
            domain,
            resource,
            owns,
            reason,
        } => {
            let relationship = if owns {
                RelationshipType::Owns
            } else {
                RelationshipType::Uses
            };
            let result =
                domains::add_resource(db, &domain, &resource, relationship, reason.as_deref())
                    .map(|_| ())
                    .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::DomainChanged(result));
        }
        Request::DomainRemoveResource { domain, resource } => {
            let result = domains::remove_resource(db, &domain, &resource)
                .map(|_| ())
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::DomainChanged(result));
        }
        Request::RemovalPlan(name) => {
            let response = removal::plan(db, &name)
                .map(Box::new)
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::RemovalPlan(response));
        }
        Request::RemovalExecute(name) => {
            let response = removal::execute(db, &name, Privilege::Pkexec)
                .map(Box::new)
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::RemovalDone(response));
        }
        Request::Graph(name) => {
            let response = graph::neighborhood(db, &name, GraphOptions::default())
                .and_then(|view| graph::render_png(&view.dot))
                .map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::Graph(response));
        }
        Request::Scan => {
            let mut progress = |event: ScanEvent| {
                let _ = responses.send_blocking(Response::ScanProgress(event));
            };
            let result = scan::run(db, &mut progress).map_err(|err| err.to_string());
            let _ = responses.send_blocking(Response::ScanDone(result));
        }
    }
}

fn fetch_detail(db: &Database, name: &str) -> Result<ResourceDetail, String> {
    let resource = resources::find_by_native_id(db.conn(), name)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("resource not found: {name}"))?;
    detail::gather(db, &resource).map_err(|err| err.to_string())
}
