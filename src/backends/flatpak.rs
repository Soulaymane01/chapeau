use std::process::Command;

use crate::discovery::flatpaks::{
    parse_flatpak_list_json, parse_flatpak_remotes_json, FlatpakRecord, FlatpakRefKind,
    FlatpakRemote,
};
use crate::errors::{ChapeauError, Result};

use super::flatpak_backend::FlatpakBackendTrait;

/// Flatpak CLI backend — discovers Flatpak state by executing flatpak as a subprocess.
pub struct FlatpakCliBackend;

impl FlatpakCliBackend {
    pub fn new() -> Self {
        Self
    }

    /// Run a flatpak command and return its stdout, or an error.
    fn run_flatpak(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("flatpak").args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ChapeauError::Backend("flatpak not found on PATH".into())
            } else {
                ChapeauError::Backend(format!("failed to execute flatpak: {}", e))
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            return Err(ChapeauError::Backend(format!(
                "flatpak exited with code {}: {}",
                code,
                stderr.lines().next().unwrap_or("unknown error")
            )));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| ChapeauError::Backend(format!("flatpak output is not valid UTF-8: {}", e)))
    }
}

impl Default for FlatpakCliBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatpakBackendTrait for FlatpakCliBackend {
    fn is_available(&self) -> bool {
        Command::new("flatpak")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn discover_installed(&self) -> Result<Vec<FlatpakRecord>> {
        let stdout = self.run_flatpak(&[
            "list",
            "--json",
            "--app",
            "--columns=application,name,version,arch,origin,active,branch,runtime,installation",
        ])?;
        parse_flatpak_list_json(&stdout, FlatpakRefKind::Application)
    }

    fn discover_runtimes(&self) -> Result<Vec<FlatpakRecord>> {
        let stdout = self.run_flatpak(&[
            "list",
            "--json",
            "--runtime",
            "--columns=application,name,version,arch,origin,active,branch,installation",
        ])?;
        parse_flatpak_list_json(&stdout, FlatpakRefKind::Runtime)
    }

    fn discover_remotes(&self) -> Result<Vec<FlatpakRemote>> {
        let stdout =
            self.run_flatpak(&["remotes", "--json", "--columns=name,url,title,options"])?;
        parse_flatpak_remotes_json(&stdout)
    }
}
