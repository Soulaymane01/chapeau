pub mod detail;
pub mod sidebar;

use adw::prelude::*;
use libadwaita as adw;

/// Show a modal error dialog on the window.
pub fn present_error(window: &adw::ApplicationWindow, message: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading("Something went wrong")
        .body(message)
        .build();
    dialog.add_response("close", "Close");
    dialog.set_default_response(Some("close"));
    dialog.present(Some(window));
}
