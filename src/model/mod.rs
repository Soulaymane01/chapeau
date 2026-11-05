//! Inventory domain model.
//!
//! Chapeau's product model is intentionally small:
//! an [`Item`] is either a launchable application or a controllable service,
//! organized into [`Group`]s and enriched with runtime status and ports.

pub mod group;
pub mod item;
pub mod port;
pub mod service;
pub mod status;

pub use group::{Group, GroupMember};
pub use item::{Item, ItemKind, ItemSource, Scope};
pub use port::{PortProtocol, ServicePort};
pub use service::CustomServiceDefinition;
pub use status::{AppStatus, RuntimeKind, ServiceState, ServiceStatus};
