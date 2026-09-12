//! Application service layer shared by the CLI and the GUI.
//!
//! Services return data, never formatted text, so that every frontend exposes
//! the same semantics. Presentation lives in `crate::cli` and the GUI.

pub mod detail;
pub mod drift;
pub mod overview;
pub mod scan;
