use async_channel::{Receiver, Sender};
use chapeau::services::detail::ResourceDetail;
use chapeau::services::drift::DriftReport;
use chapeau::services::explore::{self, ExploreKind, ExploreView};
use chapeau::services::overview::Overview;
use chapeau::services::scan::{self, ScanEvent, ScanOutcome};
use chapeau::services::status::StatusSummary;
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
    Scan,
}

/// Results delivered back to the GTK main loop.
pub enum Response {
    Overview(Result<Overview, String>),
    Detail(Result<Box<ResourceDetail>, String>),
    Explore(Result<Box<ExploreView>, String>),
    Status(Result<Box<StatusSummary>, String>),
    Drift(Result<Box<DriftReport>, String>),
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
                match request {
                    Request::Overview => {
                        let response = overview::build(&db).map_err(|err| err.to_string());
                        let _ = responses_tx.send_blocking(Response::Overview(response));
                    }
                    Request::Detail(name) => {
                        let response = fetch_detail(&db, &name).map(Box::new);
                        let _ = responses_tx.send_blocking(Response::Detail(response));
                    }
                    Request::Explore(kind) => {
                        let response = explore::build(&db, kind)
                            .map(Box::new)
                            .map_err(|err| err.to_string());
                        let _ = responses_tx.send_blocking(Response::Explore(response));
                    }
                    Request::Status => {
                        let response = status::summary(&db)
                            .map(Box::new)
                            .map_err(|err| err.to_string());
                        let _ = responses_tx.send_blocking(Response::Status(response));
                    }
                    Request::Drift => {
                        let response = drift::detect(&db)
                            .map(Box::new)
                            .map_err(|err| err.to_string());
                        let _ = responses_tx.send_blocking(Response::Drift(response));
                    }
                    Request::Scan => {
                        let mut progress = |event: ScanEvent| {
                            let _ = responses_tx.send_blocking(Response::ScanProgress(event));
                        };
                        let result = scan::run(&db, &mut progress).map_err(|err| err.to_string());
                        let _ = responses_tx.send_blocking(Response::ScanDone(result));
                    }
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

fn fetch_detail(db: &Database, name: &str) -> Result<ResourceDetail, String> {
    let resource = resources::find_by_native_id(db.conn(), name)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("resource not found: {name}"))?;
    detail::gather(db, &resource).map_err(|err| err.to_string())
}
