use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::discovery::packages::{
    DependencyRecord, DependencyType, InstallReason, PackageFileFacts, PackageRecord, RepoRecord,
};
use crate::errors::{ChapeauError, Result};

use super::package_backend::PackageBackend;

/// A resolved package-to-package dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDependency {
    /// The package that has the dependency.
    pub source_package: String,
    /// The package that fulfills the dependency.
    pub target_package: String,
}

/// DNF5 CLI backend — discovers packages by executing dnf5 as a subprocess.
pub struct DnfCliBackend;

impl DnfCliBackend {
    pub fn new() -> Self {
        Self
    }

    /// Discover all hard runtime dependencies across all installed packages.
    ///
    /// Uses two bulk DNF5 queries: one for requires, one for provides. Builds
    /// a capability → provider map from provides, then matches requirements
    /// against it. No per-package DNF5 calls — everything is bulk.
    ///
    /// Returns resolved (source_package, target_package) pairs where both are
    /// installed packages. Only hard runtime Requires are included — Recommends,
    /// Suggests, Supplements, Enhances, Provides, Conflicts, and Obsoletes are
    /// excluded. RPM-internal dependencies (rpmlib(), rtld()), config() deps,
    /// file paths, and virtual capabilities are also excluded.
    pub fn discover_all_dependencies(
        &self,
        installed_packages: &[PackageRecord],
    ) -> Result<Vec<ResolvedDependency>> {
        // Build a set of installed package names for fast lookup.
        let installed_names: HashSet<&str> =
            installed_packages.iter().map(|p| p.name.as_str()).collect();

        // Step 1: Bulk query all requires in one DNF5 call (~0.7s).
        let requires_stdout = self.run_dnf5(&[
            "repoquery",
            "--installed",
            "--queryformat",
            "%{name}\t%{requires}\n---\n",
        ])?;

        // Step 2: Bulk query all provides in one DNF5 call (~0.7s).
        let provides_stdout = self.run_dnf5(&[
            "repoquery",
            "--installed",
            "--queryformat",
            "%{name}\t%{provides}\n---\n",
        ])?;

        // Step 3: Build capability → provider map from provides output.
        let provider_map = build_provider_map(&provides_stdout);

        // Step 4: Parse requires into (package, requirement) pairs.
        let raw_pairs = parse_bulk_requires_output(&requires_stdout);

        // Step 5: Resolve each requirement to a target package.
        let mut resolved = Vec::new();
        for (source, requirement) in &raw_pairs {
            if is_excluded_requirement(requirement) {
                continue;
            }

            if is_capability(requirement) {
                // Capability requirement — look up in provider map.
                let base = capability_base(requirement);
                if let Some(target) = provider_map.get(&base) {
                    if target != source {
                        resolved.push(ResolvedDependency {
                            source_package: source.clone(),
                            target_package: target.clone(),
                        });
                    }
                }
            } else if let Some(base_name) = extract_package_name(requirement) {
                // Package-name requirement — match directly against installed packages.
                if installed_names.contains(base_name.as_str()) && base_name != *source {
                    resolved.push(ResolvedDependency {
                        source_package: source.clone(),
                        target_package: base_name,
                    });
                }
            }
        }

        Ok(resolved)
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
        // Rich package metadata in one bulk query: identity, install reason,
        // origin, install time, RPM summary and source RPM.
        let stdout = self.run_dnf5(&[
            "repoquery",
            "--installed",
            "--queryformat",
            "%{name}\t%{evr}\t%{arch}\t%{repoid}\t%{reason}\t%{from_repo}\t%{installtime}\t%{summary}\t%{sourcerpm}\n",
        ])?;
        let mut packages = parse_repoquery_output(&stdout);

        // File-payload facts in a second bulk query. The output is large
        // (~30 MB for a full Fedora install) but is reduced to a handful of
        // booleans per package and never stored.
        let files_stdout = self.run_dnf5(&[
            "repoquery",
            "--installed",
            "--queryformat",
            "@@@%{name}\n%{files}\n",
        ])?;
        let file_scan = parse_file_facts(&files_stdout);
        for pkg in &mut packages {
            pkg.file_facts = file_scan.facts.get(&pkg.name).copied().unwrap_or_default();
            pkg.service_units = file_scan
                .service_units
                .get(&pkg.name)
                .cloned()
                .unwrap_or_default();
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

    fn removal_argv(&self, package_name: &str) -> Vec<String> {
        vec![
            "dnf5".to_string(),
            "remove".to_string(),
            "-y".to_string(),
            package_name.to_string(),
        ]
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
            .is_some_and(|c| c.is_ascii_digit())
        {
            if let Some(name_pos) = before_release.rfind('-') {
                let version_part = &before_release[name_pos + 1..];
                if version_part
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
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
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
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
        let from_repo = fields
            .get(5)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let install_time = fields.get(6).and_then(|s| s.parse::<i64>().ok());
        let summary = fields
            .get(7)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let source_rpm = fields
            .get(8)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        packages.push(PackageRecord {
            name,
            version,
            arch,
            repository,
            reason,
            from_repo,
            install_time,
            summary,
            source_rpm,
            file_facts: PackageFileFacts::default(),
            service_units: Vec::new(),
        });
    }
    packages
}

/// Parse the file-list output of
/// `dnf5 repoquery --installed --queryformat '@@@%{name}\n%{files}\n'`.
///
/// The marker line introduces each package; every following line until the
/// next marker is a file path owned by that package. Only structural facts
/// are retained — never the file paths themselves.
pub fn parse_file_facts(stdout: &str) -> PackageFileScan {
    let mut facts_by_package = HashMap::new();
    let mut units_by_package: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    let mut facts = PackageFileFacts::default();

    for line in stdout.lines() {
        if let Some(name) = line.strip_prefix("@@@") {
            if let Some(previous) = current.take() {
                facts_by_package.insert(previous, facts);
            }
            current = Some(name.to_string());
            facts = PackageFileFacts::default();
            continue;
        }

        if current.is_none() || line.is_empty() {
            continue;
        }

        facts.has_files = true;
        if is_executable_path(line) {
            facts.has_executable = true;
        }
        if line.ends_with(".desktop") && line.contains("/applications/") {
            facts.has_desktop_entry = true;
        }
        if line.starts_with("/opt/") && !line.ends_with('/') {
            facts.has_app_bundle = true;
        }
        if let (Some(unit), Some(package)) = (service_unit_from_path(line), current.as_ref()) {
            units_by_package
                .entry(package.clone())
                .or_default()
                .push(unit);
        }
    }

    if let Some(previous) = current.take() {
        facts_by_package.insert(previous, facts);
    }

    PackageFileScan {
        facts: facts_by_package,
        service_units: units_by_package,
    }
}

/// The result of scanning one package file list: structural entry-point facts
/// plus the systemd unit files the package ships.
#[derive(Debug, Default)]
pub struct PackageFileScan {
    pub facts: HashMap<String, PackageFileFacts>,
    pub service_units: HashMap<String, Vec<String>>,
}

/// Extract a systemd unit file name from an owned path, if it is one.
///
/// Only unit files the package would install for the system or the user are
/// considered; drop-in directories and symlinks in `*.wants/` are ignored.
fn service_unit_from_path(path: &str) -> Option<String> {
    const UNIT_DIRS: [&str; 3] = [
        "/usr/lib/systemd/system/",
        "/usr/lib/systemd/user/",
        "/etc/systemd/system/",
    ];
    const UNIT_SUFFIXES: [&str; 10] = [
        ".service",
        ".socket",
        ".timer",
        ".target",
        ".path",
        ".mount",
        ".automount",
        ".slice",
        ".scope",
        ".swap",
    ];

    let directory = UNIT_DIRS.iter().find(|dir| path.starts_with(**dir))?;
    let name = &path[directory.len()..];
    // Ignore nested drop-in/wants directories; only top-level unit files.
    if name.is_empty() || name.contains('/') {
        return None;
    }
    if UNIT_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) {
        Some(name.to_string())
    } else {
        None
    }
}

/// True when a path is a user-executable binary in a standard bin directory.
fn is_executable_path(path: &str) -> bool {
    path.starts_with("/usr/bin/")
        || path.starts_with("/usr/sbin/")
        || path.starts_with("/bin/")
        || path.starts_with("/sbin/")
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

// --- Bulk dependency parsing helpers ---

/// Parse the bulk output of `dnf5 repoquery --installed --queryformat '%{name}\t%{requires}\n---\n'`.
///
/// The format groups requirements under each package name:
/// ```text
/// package_name\trequire1
/// require2
/// require3
///
/// ---
/// next_package\trequire1
/// ...
/// ```
///
/// The package name appears only on the first line of each group (tab-separated).
/// Subsequent lines contain only the requirement string.
pub fn parse_bulk_requires_output(stdout: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_package: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim().to_string();

        // Group separator — reset current package.
        if line == "---" {
            current_package = None;
            continue;
        }

        if line.is_empty() {
            current_package = None;
            continue;
        }

        // Check if this line contains a tab (first line of a group).
        if let Some(tab_pos) = line.find('\t') {
            let package = line[..tab_pos].to_string();
            let requirement = line[tab_pos + 1..].to_string();
            current_package = Some(package.clone());
            if !requirement.is_empty() {
                pairs.push((package, requirement));
            }
        } else if let Some(ref package) = current_package {
            // Continuation line — requirement without package name.
            pairs.push((package.clone(), line));
        }
    }

    pairs
}

/// Build a capability → provider package name map from DNF5 provides output.
///
/// Parses the `%{name}\t%{provides}\n---\n` bulk format and maps each
/// .so capability to the package that provides it. Only maps capabilities
/// (requirements containing `.so`), not package-name provides.
fn build_provider_map(stdout: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_package: Option<String> = None;

    for line in stdout.lines() {
        if line == "---" {
            current_package = None;
            continue;
        }

        if line.contains('\t') {
            let tab_pos = line.find('\t').unwrap();
            let package = line[..tab_pos].to_string();
            let provides = line[tab_pos + 1..].to_string();
            current_package = Some(package.clone());
            // Only map .so capabilities.
            if provides.contains(".so") {
                let base = capability_base(&provides);
                map.entry(base).or_insert(package);
            }
        } else if let Some(ref package) = current_package {
            // Continuation line — provides without package name.
            if line.contains(".so") {
                let base = capability_base(line);
                map.entry(base).or_insert(package.clone());
            }
        }
    }

    map
}

/// Check if a requirement should be excluded from dependency tracking.
///
/// Excluded categories:
/// - `rpmlib(...)` — RPM internal capabilities
/// - `rtld(...)` — dynamic linker internals
/// - `config(...)` — config file dependencies
/// - File paths (starting with `/`)
/// - `filesystem(...)` virtual capabilities
fn is_excluded_requirement(requirement: &str) -> bool {
    requirement.starts_with("rpmlib(")
        || requirement.starts_with("rtld(")
        || requirement.starts_with("config(")
        || requirement.starts_with('/')
        || requirement.starts_with("filesystem(")
}

/// Check if a requirement is a capability (shared library) rather than a package name.
///
/// Capabilities typically contain `.so` (shared library files).
fn is_capability(requirement: &str) -> bool {
    // Strip version constraint suffix (e.g. ">= 2.42" or "= 3.14.6").
    let base = requirement.split_whitespace().next().unwrap_or(requirement);
    base.contains(".so")
}

/// Check if a string is a known RPM architecture name.
fn is_known_arch(arch: &str) -> bool {
    matches!(
        arch,
        "x86-64"
            | "i686"
            | "i586"
            | "i486"
            | "i386"
            | "aarch64"
            | "armv7hl"
            | "armv6hl"
            | "noarch"
            | "ppc64le"
            | "s390x"
    )
}

/// Extract the base capability name, stripping version suffixes like `(GLIBC_2.14)`.
///
/// For example: `libc.so.6(GLIBC_2.14)(64bit)` → `libc.so.6()(64bit)`
fn capability_base(capability: &str) -> String {
    // Strip version constraint if present (e.g. ">= 2.42").
    let base = capability.split_whitespace().next().unwrap_or(capability);
    // Replace version-qualified parentheses with empty ones.
    // Version suffixes like (GLIBC_2.14), (GCC_3.0), (CXXABI_1.3), (GLIBCXX_3.4),
    // (GLIBC_ABI_DT_RELR), (GLIBC_PRIVATE) replace the empty () in the base capability.
    let mut result = base.to_string();
    for prefix in &[
        "(GLIBC_ABI_",
        "(GLIBC_PRIVATE",
        "(GLIBC_",
        "(GCC_",
        "(CXXABI_",
        "(GLIBCXX_",
    ] {
        while let Some(start) = result.find(prefix) {
            if let Some(end) = result[start..].find(')') {
                result = format!("{}(){}", &result[..start], &result[start + end + 1..]);
            } else {
                break;
            }
        }
    }
    result
}

/// Extract a package name from a requirement string that looks like a package-name requirement.
///
/// Handles formats like:
/// - `python3-libs(x86-64) = 3.14.6-1.fc44` → `python3-libs`
/// - `filesystem >= 3` → `filesystem`
/// - `glibc >= 2.42.9000-22` → `glibc`
/// - `bash` → `bash`
///
/// Returns `None` if the requirement doesn't look like a package name.
fn extract_package_name(requirement: &str) -> Option<String> {
    let base = requirement.split_whitespace().next()?;

    // Skip if it looks like a capability (contains .so).
    if base.contains(".so") {
        return None;
    }

    // Handle architecture suffixes and virtual capabilities.
    // Package names: `python3-libs(x86-64) = 3.14.6-1.fc44` → `python3-libs`
    // Virtual capabilities: `config(bash) = 5.3.9-3.fc44` → skip (the part in parens is not an arch).
    if let Some(paren_pos) = base.find('(') {
        let arch = &base[paren_pos + 1..];
        // Architecture suffix ends with `)`. Check if the part inside looks like a real arch.
        if let Some(close_pos) = arch.find(')') {
            let arch_candidate = &arch[..close_pos];
            if is_known_arch(arch_candidate) {
                // Real package name with arch suffix — strip it.
                let name = &base[..paren_pos];
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
                {
                    return Some(name.to_string());
                }
            }
        }
        // Virtual capability (e.g. config(bash)) — skip.
        return None;
    }

    let name = base;

    // Must be a non-empty alphanumeric name (with hyphens/underscores/dots).
    // No parentheses allowed — those indicate virtual capabilities like config().
    if name.is_empty()
        || name.contains('(')
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return None;
    }

    Some(name.to_string())
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

    // --- Bulk requires parsing tests ---

    #[test]
    fn parse_bulk_requires_simple() {
        let input =
            "bash\tfilesystem >= 3\nlibc.so.6()(64bit)\n---\ncurl\tlibc.so.6()(64bit)\n---\n";
        let pairs = parse_bulk_requires_output(input);
        assert_eq!(pairs.len(), 3);
        assert_eq!(
            pairs[0],
            ("bash".to_string(), "filesystem >= 3".to_string())
        );
        assert_eq!(
            pairs[1],
            ("bash".to_string(), "libc.so.6()(64bit)".to_string())
        );
        assert_eq!(
            pairs[2],
            ("curl".to_string(), "libc.so.6()(64bit)".to_string())
        );
    }

    #[test]
    fn parse_bulk_requires_multi_line() {
        let input = "bash\t/usr/bin/sh\nconfig(bash) = 5.3.9-3.fc44\nfilesystem >= 3\nlibc.so.6()(64bit)\n---\n";
        let pairs = parse_bulk_requires_output(input);
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs[0].0, "bash");
        assert_eq!(pairs[0].1, "/usr/bin/sh");
        assert_eq!(pairs[1].1, "config(bash) = 5.3.9-3.fc44");
    }

    #[test]
    fn parse_bulk_requires_empty() {
        let pairs = parse_bulk_requires_output("");
        assert!(pairs.is_empty());
    }

    // --- Exclusion/filtering tests ---

    #[test]
    fn is_excluded_requirement_tests() {
        assert!(is_excluded_requirement(
            "rpmlib(CompressedFileNames) <= 3.0.4-1"
        ));
        assert!(is_excluded_requirement("rtld(GNU_HASH)"));
        assert!(is_excluded_requirement("config(bash) = 5.3.9-3.fc44"));
        assert!(is_excluded_requirement("/usr/bin/sh"));
        assert!(is_excluded_requirement("/bin/sh"));
        assert!(is_excluded_requirement(
            "filesystem(unmerged-sbin-symlinks)"
        ));
        assert!(!is_excluded_requirement("libc.so.6()(64bit)"));
        assert!(!is_excluded_requirement("glibc >= 2.42.9000-22"));
        assert!(!is_excluded_requirement("filesystem >= 3"));
    }

    #[test]
    fn is_capability_tests() {
        assert!(is_capability("libc.so.6()(64bit)"));
        assert!(is_capability("libgcc_s.so.1()(64bit)"));
        assert!(is_capability("libtinfo.so.6()(64bit)"));
        assert!(!is_capability("glibc >= 2.42.9000-22"));
        assert!(!is_capability("filesystem >= 3"));
        assert!(!is_capability("bash"));
    }

    #[test]
    fn capability_base_strips_version_suffixes() {
        assert_eq!(
            capability_base("libc.so.6(GLIBC_2.14)(64bit)"),
            "libc.so.6()(64bit)"
        );
        assert_eq!(
            capability_base("libgcc_s.so.1(GCC_3.0)(64bit)"),
            "libgcc_s.so.1()(64bit)"
        );
        assert_eq!(
            capability_base("libstdc++.so.6(CXXABI_1.3)(64bit)"),
            "libstdc++.so.6()(64bit)"
        );
        assert_eq!(
            capability_base("libstdc++.so.6(GLIBCXX_3.4)(64bit)"),
            "libstdc++.so.6()(64bit)"
        );
        assert_eq!(capability_base("libc.so.6()(64bit)"), "libc.so.6()(64bit)");
    }

    #[test]
    fn extract_package_name_tests() {
        assert_eq!(
            extract_package_name("glibc >= 2.42.9000-22"),
            Some("glibc".to_string())
        );
        assert_eq!(
            extract_package_name("filesystem >= 3"),
            Some("filesystem".to_string())
        );
        assert_eq!(
            extract_package_name("python3-libs(x86-64) = 3.14.6-1.fc44"),
            Some("python3-libs".to_string())
        );
        assert_eq!(extract_package_name("bash"), Some("bash".to_string()));
        assert_eq!(extract_package_name("libc.so.6()(64bit)"), None);
        assert_eq!(extract_package_name("/usr/bin/sh"), None);
        // config() deps are excluded by is_excluded_requirement before reaching this,
        // but extract_package_name also rejects names with parentheses.
        assert_eq!(extract_package_name("config(bash) = 5.3.9-3.fc44"), None);
    }
}
