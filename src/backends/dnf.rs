use serde::Deserialize;
use std::process::Command;

use crate::discovery::packages::{
    DependencyRecord, DependencyType, InstallReason, PackageRecord, RepoRecord,
};
use crate::errors::{ChapeauError, Result};

use super::package_backend::PackageBackend;

/// DNF5 CLI backend — discovers packages by executing dnf5 as a subprocess.
pub struct DnfCliBackend;

impl DnfCliBackend {
    pub fn new() -> Self {
        Self
    }

    /// Run a dnf5 command and return its stdout, or an error.
    fn run_dnf5(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("dnf5").args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ChapeauError::Backend("dnf5 not found on PATH".into())
            } else {
                ChapeauError::Backend(format!("failed to execute dnf5: {}", e))
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            return Err(ChapeauError::Backend(format!(
                "dnf5 exited with code {}: {}",
                code,
                stderr.lines().next().unwrap_or("unknown error")
            )));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| ChapeauError::Backend(format!("dnf5 output is not valid UTF-8: {}", e)))
    }
}

impl Default for DnfCliBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageBackend for DnfCliBackend {
    fn is_available(&self) -> bool {
        Command::new("dnf5")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn discover_installed(&self) -> Result<Vec<PackageRecord>> {
        // Use repoquery with queryformat for rich data (name, version, arch, repo, reason, from_repo, installtime).
        let stdout = self.run_dnf5(&[
            "repoquery",
            "--installed",
            "--queryformat",
            "%{name}\t%{evr}\t%{arch}\t%{repoid}\t%{reason}\t%{from_repo}\t%{installtime}\n",
        ])?;

        let mut packages = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 4 {
                continue; // skip malformed lines
            }
            let name = fields[0].to_string();
            let version = fields[1].to_string();
            let arch = fields[2].to_string();
            let repository = fields[3].to_string();
            let reason = if fields.len() > 4 {
                InstallReason::from_dnf5(fields[4])
            } else {
                InstallReason::Unknown
            };
            let from_repo = if fields.len() > 5 && !fields[5].is_empty() {
                Some(fields[5].to_string())
            } else {
                None
            };
            let install_time = if fields.len() > 6 && !fields[6].is_empty() {
                fields[6].parse::<i64>().ok()
            } else {
                None
            };

            packages.push(PackageRecord {
                name,
                version,
                arch,
                repository,
                reason,
                from_repo,
                install_time,
            });
        }

        Ok(packages)
    }

    fn discover_repositories(&self) -> Result<Vec<RepoRecord>> {
        let stdout = self.run_dnf5(&["repo", "list", "--json"])?;

        let repos: Vec<RawRepo> = serde_json::from_str(&stdout).map_err(|e| {
            ChapeauError::Backend(format!("failed to parse dnf5 repo list JSON: {}", e))
        })?;

        Ok(repos
            .into_iter()
            .map(|r| RepoRecord {
                id: r.id,
                name: r.name,
                is_enabled: r.is_enabled,
            })
            .collect())
    }

    fn query_dependencies(&self, package_name: &str) -> Result<Vec<DependencyRecord>> {
        let stdout = self.run_dnf5(&["repoquery", "--installed", "--requires", package_name])?;

        Ok(stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| DependencyRecord {
                source_package: package_name.to_string(),
                dependency_string: line.trim().to_string(),
                dep_type: DependencyType::Requires,
            })
            .collect())
    }

    fn query_reverse_dependencies(&self, package_name: &str) -> Result<Vec<String>> {
        let stdout = self.run_dnf5(&["repoquery", "--installed", "--whatdepends", package_name])?;

        Ok(stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                // Output is NEVRA format: "name-epoch:version-release.arch"
                // Extract just the package name (everything before the last '-' that precedes epoch).
                parse_nevra_name(line.trim())
            })
            .filter(|name| !name.is_empty())
            .collect())
    }
}

// --- JSON deserialization helpers ---

#[derive(Deserialize)]
struct RawRepo {
    id: String,
    name: String,
    is_enabled: bool,
}

// --- NEVRA parsing ---

/// Extract the package name from a NEVRA string like "bash-5.3.9-3.fc44.x86_64".
/// Extract the package name from a NEVRA string.
/// NEVRA format: name-epoch:version-release.arch (epoch optional)
/// The name is everything before the version-release segment.
/// Version-release is always separated by a `-`, and the release never contains `-`.
/// So we find the last `-` (release boundary) and the one before it (name/version boundary).
pub fn parse_nevra_name(nevra: &str) -> String {
    // Strip known arch suffix.
    let without_arch = if let Some(pos) = nevra.rfind('.') {
        let suffix = &nevra[pos..];
        if matches!(
            suffix,
            ".x86_64"
                | ".i686"
                | ".aarch64"
                | ".noarch"
                | ".i586"
                | ".ppc64le"
                | ".s390x"
                | ".armv7hl"
        ) {
            &nevra[..pos]
        } else {
            nevra
        }
    } else {
        nevra
    };

    // RPM NEVRA: name-version-release. The release never contains '-'.
    // Find the last '-' (release boundary), then the one before it (name/version boundary).
    // Both version and release must start with a digit for this to be valid.
    if let Some(release_pos) = without_arch.rfind('-') {
        let release_part = &without_arch[release_pos + 1..];
        let before_release = &without_arch[..release_pos];
        if release_part
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_digit())
        {
            if let Some(name_pos) = before_release.rfind('-') {
                let version_part = &before_release[name_pos + 1..];
                if version_part
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_ascii_digit())
                {
                    return without_arch[..name_pos].to_string();
                }
            }
        }
    }
    without_arch.to_string()
}

// --- Public parsing functions (for use by tests and other modules) ---

/// Parse the tab-separated output of `dnf5 repoquery --installed --queryformat`.
pub fn parse_repoquery_output(stdout: &str) -> Vec<PackageRecord> {
    let mut packages = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            continue;
        }
        let name = fields[0].to_string();
        let version = fields[1].to_string();
        let arch = fields[2].to_string();
        let repository = fields[3].to_string();
        let reason = if fields.len() > 4 {
            InstallReason::from_dnf5(fields[4])
        } else {
            InstallReason::Unknown
        };
        let from_repo = if fields.len() > 5 && !fields[5].is_empty() {
            Some(fields[5].to_string())
        } else {
            None
        };
        let install_time = if fields.len() > 6 && !fields[6].is_empty() {
            fields[6].parse::<i64>().ok()
        } else {
            None
        };

        packages.push(PackageRecord {
            name,
            version,
            arch,
            repository,
            reason,
            from_repo,
            install_time,
        });
    }
    packages
}

/// Parse the JSON output of `dnf5 repo list --json`.
pub fn parse_repo_list_json(json: &str) -> Result<Vec<RepoRecord>> {
    let repos: Vec<RawRepo> = serde_json::from_str(json)
        .map_err(|e| ChapeauError::Backend(format!("invalid repo list JSON: {}", e)))?;
    Ok(repos
        .into_iter()
        .map(|r| RepoRecord {
            id: r.id,
            name: r.name,
            is_enabled: r.is_enabled,
        })
        .collect())
}

/// Parse the text output of `dnf5 repoquery --installed --requires <pkg>`.
pub fn parse_requires_output(package_name: &str, stdout: &str) -> Vec<DependencyRecord> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| DependencyRecord {
            source_package: package_name.to_string(),
            dependency_string: line.trim().to_string(),
            dep_type: DependencyType::Requires,
        })
        .collect()
}

/// Parse the text output of `dnf5 repoquery --installed --whatdepends <pkg>`.
pub fn parse_whatdepends_output(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| parse_nevra_name(line.trim()))
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nevra_simple() {
        assert_eq!(parse_nevra_name("bash-5.3.9-3.fc44.x86_64"), "bash");
    }

    #[test]
    fn parse_nevra_with_epoch() {
        assert_eq!(
            parse_nevra_name("ImageMagick-1:7.1.2.13-2.fc44.x86_64"),
            "ImageMagick"
        );
    }

    #[test]
    fn parse_nevra_noarch() {
        assert_eq!(
            parse_nevra_name("filesystem-3.18-4.fc44.noarch"),
            "filesystem"
        );
    }

    #[test]
    fn parse_nevra_hyphenated_name() {
        assert_eq!(
            parse_nevra_name("NetworkManager-1:1.56.1-2.fc44.x86_64"),
            "NetworkManager"
        );
    }

    #[test]
    fn parse_nevra_lib64() {
        assert_eq!(parse_nevra_name("glibc-2.40-3.fc44.x86_64"), "glibc");
    }
}
