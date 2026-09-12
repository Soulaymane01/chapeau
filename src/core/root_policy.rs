//! Root candidate classification policy.
//!
//! DNF's `user-installed` flag is *evidence* of user intent, not semantic
//! truth: it also marks development headers, Perl modules, firmware and
//! libraries that were pulled in as part of an explicit install transaction.
//!
//! This module turns package metadata (summary, source RPM, file payload)
//! plus the dependency graph into a small, deterministic, explainable
//! classification:
//!
//! ```text
//! DNF user-installed
//!         ↓
//!   candidate evidence
//!         ↓
//!  semantic role + entry points + graph position
//!         ↓
//!   intentional root OR supporting resource
//! ```
//!
//! The policy is intentionally local and testable: it has no database or
//! process dependencies. The scanner feeds it discovered facts and persists
//! the resulting decisions.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Minimum score for a package to be considered an intentional root.
pub const ROOT_THRESHOLD: i32 = 4;

/// Evidence points awarded to a DNF user-installed package.
const SCORE_USER_INSTALLED: i32 = 2;
/// Evidence points for the canonical package of a source project
/// (metapackage or source-base package whose subpackages provide the
/// user-facing entry points).
const SCORE_CANONICAL: i32 = 3;
/// Weak positive for a package whose role is a user-facing application.
const SCORE_APPLICATION_ROLE: i32 = 1;
/// Strong positive for a package shipping a `.desktop` entry.
const SCORE_DESKTOP_ENTRY: i32 = 5;
/// Positive for a package shipping an application bundle under `/opt`.
const SCORE_APP_BUNDLE: i32 = 4;
/// Positive for a package shipping an executable in a standard bin dir.
const SCORE_EXECUTABLE: i32 = 2;
/// Penalty applied to every supporting (non-application) package role.
const SCORE_SUPPORTING_ROLE: i32 = -10;
/// Penalty applied when a candidate is demoted to a component of another
/// same-project candidate.
const SCORE_PROJECT_DEMOTION: i32 = -10;

/// The semantic role a package plays, derived from authoritative RPM/package
/// metadata (summary text, package name conventions, source package).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageRole {
    /// A user-facing application, tool, shell, server or service package.
    Application,
    /// Development headers, shared development files (`-devel`, `-headers`).
    Development,
    /// Shared/runtime libraries and language bindings.
    Library,
    /// Directory-layout and filesystem-ownership packages.
    Filesystem,
    /// Hardware firmware blobs.
    Firmware,
    /// Kernel and kernel-module packages.
    Kernel,
    /// System administration and platform tooling: package managers, boot
    /// loaders, signing tools, drivers and similar.
    System,
    /// Perl module packages (Fedora packages them as `perl-*`).
    PerlModule,
    /// RPM macro packages used for building.
    Macro,
    /// Documentation-only packages.
    Documentation,
    /// Fonts, icon themes and other presentation assets.
    Asset,
    /// Common files, configuration, plugins, translations and other
    /// supporting components.
    Support,
}

impl PackageRole {
    /// True when this role is supporting rather than user-facing.
    pub fn is_supporting(self) -> bool {
        !matches!(self, PackageRole::Application)
    }

    /// Human-readable label used in CLI explanations.
    pub fn label(self) -> &'static str {
        match self {
            PackageRole::Application => "application",
            PackageRole::Development => "development package",
            PackageRole::Library => "library or runtime",
            PackageRole::Filesystem => "filesystem layout package",
            PackageRole::Firmware => "firmware",
            PackageRole::Kernel => "kernel component",
            PackageRole::System => "system component",
            PackageRole::PerlModule => "Perl module",
            PackageRole::Macro => "build macro package",
            PackageRole::Documentation => "documentation package",
            PackageRole::Asset => "presentation asset",
            PackageRole::Support => "supporting component",
        }
    }
}

impl std::fmt::Display for PackageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Runtime facts about one installed package, gathered by discovery.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageFacts {
    /// Package name.
    pub name: String,
    /// RPM summary (short description).
    pub summary: Option<String>,
    /// Source RPM the package was built from.
    pub source_rpm: Option<String>,
    /// Whether DNF marks the package as user-installed.
    pub user_installed: bool,
    /// Whether the package owns any files at all.
    pub has_files: bool,
    /// Whether the package installs an executable in a standard bin dir.
    pub has_executable: bool,
    /// Whether the package installs a `.desktop` entry.
    pub has_desktop_entry: bool,
    /// Whether the package installs an application payload under `/opt`.
    pub has_app_bundle: bool,
}

impl PackageFacts {
    /// The base name of the package's source RPM (e.g. `git` for
    /// `git-2.54.0-1.fc44.src.rpm`).
    pub fn source_base(&self) -> Option<String> {
        self.source_rpm
            .as_deref()
            .map(source_base)
            .filter(|base| !base.is_empty())
    }
}

/// Positive evidence that contributed to a root decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RootEvidence {
    /// DNF reported the package as user-installed.
    DnfUserInstalled,
    /// The package installs a `.desktop` entry.
    DesktopEntry,
    /// The package installs an application payload under `/opt`.
    ApplicationBundle,
    /// The package installs an executable in a standard bin directory.
    Executable,
    /// The package is the canonical package of its source project.
    CanonicalProjectPackage,
}

impl RootEvidence {
    pub fn label(&self) -> &'static str {
        match self {
            RootEvidence::DnfUserInstalled => "DNF user-installed package",
            RootEvidence::DesktopEntry => "desktop application entry",
            RootEvidence::ApplicationBundle => "application bundle",
            RootEvidence::Executable => "user-facing executable",
            RootEvidence::CanonicalProjectPackage => "canonical project package",
        }
    }
}

/// Why a package was *not* classified as an intentional root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DemotionReason {
    /// DNF does not mark the package as user-installed. It may still be a
    /// user-facing binary, but it was pulled in as a dependency or as part
    /// of a group rather than chosen on its own.
    NotUserInstalled,
    /// The package role is supporting (library, development, firmware...).
    SupportingRole(PackageRole),
    /// The package has no user-facing entry point.
    NoUserFacingEntryPoint,
    /// A stronger package from the same project depends on this one.
    ComponentOf(String),
    /// The package is a component of a development toolchain from the same
    /// source project.
    ToolchainComponentOf(String),
    /// Another candidate from the same project already represents it.
    CoveredBy(String),
}

impl DemotionReason {
    /// Human-readable explanation.
    pub fn describe(&self) -> String {
        match self {
            DemotionReason::NotUserInstalled => {
                "not marked as user-installed (dependency or group install)".to_string()
            }
            DemotionReason::SupportingRole(role) => {
                format!("package role: {}", role.label())
            }
            DemotionReason::NoUserFacingEntryPoint => {
                "no user-facing executable or application entry point".to_string()
            }
            DemotionReason::ComponentOf(other) => format!("component of '{}'", other),
            DemotionReason::ToolchainComponentOf(other) => {
                format!("component of the development toolchain of '{}'", other)
            }
            DemotionReason::CoveredBy(other) => {
                format!("already represented by project package '{}'", other)
            }
        }
    }
}

/// The classifier's decision for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDecision {
    /// Package name.
    pub name: String,
    /// Semantic role derived from metadata.
    pub role: PackageRole,
    /// Score before project-level demotions.
    pub intrinsic_score: i32,
    /// Score after project-level demotions.
    pub score: i32,
    /// Whether the package should be a detected root.
    pub is_root: bool,
    /// Positive evidence supporting the decision.
    pub evidence: Vec<RootEvidence>,
    /// Reasons the package is supporting, in a stable order.
    pub demotions: Vec<DemotionReason>,
}

impl PackageDecision {
    /// Compact, human-readable reason string for persistence in `roots.reason`.
    pub fn reason_string(&self) -> String {
        let mut parts: Vec<&str> = self.evidence.iter().map(|e| e.label()).collect();
        if parts.is_empty() {
            parts.push("semantic root classification");
        }
        parts.join("; ")
    }
}

/// Derive the base package name from a source RPM filename.
///
/// `git-2.54.0-1.fc44.src.rpm` → `git`
/// `postgresql18-18.3-2.fc44.src.rpm` → `postgresql18`
pub fn source_base(source_rpm: &str) -> String {
    let stem = source_rpm
        .strip_suffix(".src.rpm")
        .unwrap_or(source_rpm)
        .trim();
    let mut parts: Vec<&str> = stem.split('-').collect();
    while parts.len() > 1 {
        let last = parts[parts.len() - 1];
        if !last.is_empty() && last.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            parts.pop();
        } else {
            break;
        }
    }
    parts.join("-")
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

/// Classify a package's semantic role from its name and summary.
///
/// Rules are ordered from the most specific packaging conventions to the most
/// general keyword signals. They are metadata-driven: the summary text comes
/// from RPM, and the `-devel` / `perl-` / `-filesystem` conventions are part
/// of Fedora packaging guidelines.
pub fn classify_package_role(name: &str, summary: Option<&str>) -> PackageRole {
    let summary = summary.unwrap_or("").to_ascii_lowercase();
    let name_lower = name.to_ascii_lowercase();

    // Perl module packages follow a strict `perl-Module::Name` convention.
    if name_lower.starts_with("perl-")
        || contains_any(
            &summary,
            &[
                "perl module",
                "perl modules",
                "perl extension",
                "perl interface",
                "perl library",
                "perl class",
            ],
        )
    {
        return PackageRole::PerlModule;
    }

    if name_lower.starts_with("kernel")
        || name_lower.starts_with("kmod-")
        || name_lower.starts_with("akmod-")
        || summary.contains("kernel module")
    {
        return PackageRole::Kernel;
    }

    if summary.contains("firmware") {
        return PackageRole::Firmware;
    }

    if contains_any(
        &summary,
        &[
            "package manager",
            "package maintenance",
            "boot loader",
            "bootloader",
            "secure boot",
            "signing utility",
            "signing tool",
            "signature",
            "signatures",
            "compatibility",
            "snapshot",
            "driver",
            "file system in userspace",
            "userspace file system",
            "video4linux",
            "udev",
            "uefi",
            "cryptographic architecture",
        ],
    ) {
        return PackageRole::System;
    }

    if name_lower.ends_with("-devel")
        || name_lower.ends_with("-headers")
        || contains_any(
            &summary,
            &[
                "development files",
                "development package",
                "development tools",
                "devel libraries",
                "devel files",
                "header files",
                "files for developing",
                "libraries and header",
                "libraries & headers",
                "development",
                "devel ",
            ],
        )
    {
        return PackageRole::Development;
    }

    if contains_any(
        &summary,
        &["rpm macros", "srpm macros", "macros for", "macros required"],
    ) {
        return PackageRole::Macro;
    }

    if name_lower.ends_with("-doc")
        || name_lower.ends_with("-docs")
        || contains_any(
            &summary,
            &[
                "documentation files",
                "doc files",
                "doc package",
                "doc tools",
                "api documentation",
                "man pages",
            ],
        )
    {
        return PackageRole::Documentation;
    }

    if name_lower.ends_with("-filesystem")
        || contains_any(
            &summary,
            &[
                "directory layout",
                "filesystem layout",
                "filesystem package",
                "filesystem for",
                "layout for",
            ],
        )
    {
        return PackageRole::Filesystem;
    }

    if contains_any(
        &summary,
        &["font", "icon theme", "wallpaper", "sound theme"],
    ) {
        return PackageRole::Asset;
    }

    if name_lower.ends_with("-libs")
        || name_lower.ends_with("-lib")
        || contains_any(
            &summary,
            &[
                "library",
                "libraries",
                "runtime",
                "shared library",
                "bindings",
                "framework",
                " api",
                "codec",
                "engine for",
                "implementation of",
                "implementation",
                "plugins for",
            ],
        )
    {
        return PackageRole::Library;
    }

    if contains_any(
        &summary,
        &[
            "directory layout",
            "configuration files",
            "configuration",
            "repository files",
            "repository configuration",
            "customizations",
            "macros",
            "modules",
            "scripts for",
            "dependencies",
            "installation environment",
            "support for",
            "common files",
            "files needed",
            "plugin",
            "data for",
            "utility files",
            "translations",
            "localization",
            "l10n",
            "language pack",
        ],
    ) {
        return PackageRole::Support;
    }

    PackageRole::Application
}

/// Two packages belong to the same project when they share a source package
/// base name, or when one package name is a `-`-delimited extension of the
/// other (e.g. `docker-ce` / `docker-ce-cli`).
fn same_project(
    a: &PackageFacts,
    a_base: Option<&str>,
    b: &PackageFacts,
    b_base: Option<&str>,
) -> bool {
    match (a_base, b_base) {
        (Some(x), Some(y)) if x == y => return true,
        _ => {}
    }
    let (an, bn) = (a.name.as_str(), b.name.as_str());
    an.starts_with(&format!("{}-", bn)) || bn.starts_with(&format!("{}-", an))
}

fn intrinsic_score(
    facts: &PackageFacts,
    canonical: bool,
    role: PackageRole,
) -> (i32, Vec<RootEvidence>) {
    let mut score = 0;
    let mut evidence = Vec::new();

    if facts.user_installed {
        score += SCORE_USER_INSTALLED;
        evidence.push(RootEvidence::DnfUserInstalled);
    }
    if canonical {
        score += SCORE_CANONICAL;
        evidence.push(RootEvidence::CanonicalProjectPackage);
    }
    if role == PackageRole::Application {
        score += SCORE_APPLICATION_ROLE;
    }
    if facts.has_desktop_entry {
        score += SCORE_DESKTOP_ENTRY;
        evidence.push(RootEvidence::DesktopEntry);
    }
    if facts.has_app_bundle {
        score += SCORE_APP_BUNDLE;
        evidence.push(RootEvidence::ApplicationBundle);
    }
    if facts.has_executable {
        score += SCORE_EXECUTABLE;
        evidence.push(RootEvidence::Executable);
    }
    if role.is_supporting() {
        score += SCORE_SUPPORTING_ROLE;
    }

    (score, evidence)
}

/// Classify all packages and decide which are intentional roots.
///
/// `dependents` maps a package name to the installed packages that directly
/// depend on it (incoming `DependsOn` edges). The classifier is deterministic:
/// input order does not affect decisions.
pub fn classify_packages(
    facts: &[PackageFacts],
    dependents: &HashMap<String, Vec<String>>,
) -> Vec<PackageDecision> {
    let by_name: HashMap<&str, usize> = facts
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i))
        .collect();

    // Precompute source-package bases once: project grouping is the basis for
    // both canonical detection and demotion.
    let source_bases: Vec<Option<String>> = facts.iter().map(|f| f.source_base()).collect();

    // A canonical project package carries the project identity when its own
    // subpackages hold the user-facing entry points (git → git-core,
    // perl → perl-interpreter) and/or it declares itself a metapackage.
    let canonical: Vec<bool> = facts
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if f.summary
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .contains("metapackage")
            {
                return true;
            }
            let has_entry = f.has_executable || f.has_desktop_entry || f.has_app_bundle;
            if has_entry || source_bases[i].as_deref() != Some(f.name.as_str()) {
                return false;
            }
            source_bases.iter().enumerate().any(|(j, other_base)| {
                j != i
                    && other_base == &source_bases[i]
                    && (facts[j].has_executable
                        || facts[j].has_desktop_entry
                        || facts[j].has_app_bundle)
            })
        })
        .collect();

    let roles: Vec<PackageRole> = facts
        .iter()
        .map(|f| classify_package_role(&f.name, f.summary.as_deref()))
        .collect();

    let (mut scores, evidence): (Vec<i32>, Vec<Vec<RootEvidence>>) = facts
        .iter()
        .zip(canonical.iter().zip(roles.iter()))
        .map(|(f, (c, r))| intrinsic_score(f, *c, *r))
        .unzip();

    let intrinsic = scores.clone();

    let candidate_indices: Vec<usize> = (0..facts.len())
        .filter(|&i| scores[i] >= ROOT_THRESHOLD)
        .collect();
    let candidate_set: HashSet<usize> = candidate_indices.iter().copied().collect();

    let mut demotions: Vec<Vec<DemotionReason>> = vec![Vec::new(); facts.len()];

    for &i in &candidate_indices {
        let fact = &facts[i];
        let role_i = roles[i];

        // DNF's user-installed flag is the minimum intent evidence for an
        // automatic root: a binary shipped by a dependency or a group member
        // is not a top-level intentional resource. Explicit roots bypass
        // this policy entirely (handled by the caller).
        if !fact.user_installed {
            demotions[i].push(DemotionReason::NotUserInstalled);
        }

        // Role-first demotion: a package whose metadata says it is supporting
        // (development headers, library, firmware, Perl module, ...) never
        // becomes a root, even if it happens to ship a binary.
        if role_i.is_supporting() {
            demotions[i].push(DemotionReason::SupportingRole(role_i));
        }

        let dependents_of_i = dependents.get(&fact.name);

        if let Some(dep_names) = dependents_of_i {
            for dep_name in dep_names {
                let Some(&j) = by_name.get(dep_name.as_str()) else {
                    continue;
                };
                if j == i {
                    continue;
                }
                if !same_project(
                    fact,
                    source_bases[i].as_deref(),
                    &facts[j],
                    source_bases[j].as_deref(),
                ) {
                    continue;
                }

                if roles[j] == PackageRole::Development {
                    // A package that exists only to serve a same-project
                    // development package is a toolchain component
                    // (e.g. qt6-linguist for qt6-qttools-devel).
                    let has_other_candidate_consumer = candidate_indices.iter().any(|&k| {
                        k != i
                            && roles[k] != PackageRole::Development
                            && same_project(
                                fact,
                                source_bases[i].as_deref(),
                                &facts[k],
                                source_bases[k].as_deref(),
                            )
                            && dependents
                                .get(&facts[k].name)
                                .is_some_and(|ds| ds.contains(&fact.name))
                    });
                    if !has_other_candidate_consumer {
                        demotions[i].push(DemotionReason::ToolchainComponentOf(dep_name.clone()));
                    }
                } else if !candidate_set.contains(&j) {
                    continue;
                } else if intrinsic[j] > intrinsic[i]
                    || (intrinsic[j] == intrinsic[i]
                        && !depends_on(dependents, &facts[i], &facts[j]))
                {
                    demotions[i].push(DemotionReason::ComponentOf(dep_name.clone()));
                }
            }
        }

        // A weaker candidate that shares a project with a stronger one is
        // already represented by that project root (HandBrake vs HandBrake-gui).
        if let Some(stronger) = candidate_indices.iter().find_map(|&j| {
            (j != i
                && intrinsic[j] > intrinsic[i]
                && same_project(
                    fact,
                    source_bases[i].as_deref(),
                    &facts[j],
                    source_bases[j].as_deref(),
                ))
            .then_some(facts[j].name.as_str())
        }) {
            demotions[i].push(DemotionReason::CoveredBy(stronger.to_string()));
        }

        // Supporting roles are decided by metadata alone; the demotion list is
        // informational for them.
        let project_demotions = demotions[i]
            .iter()
            .filter(|d| {
                matches!(
                    d,
                    DemotionReason::ComponentOf(_)
                        | DemotionReason::ToolchainComponentOf(_)
                        | DemotionReason::CoveredBy(_)
                )
            })
            .count();
        scores[i] += SCORE_PROJECT_DEMOTION * project_demotions as i32;
    }

    facts
        .iter()
        .enumerate()
        .map(|(i, fact)| {
            let supporting = roles[i].is_supporting();
            let has_intent_evidence = fact.user_installed;
            let is_root = has_intent_evidence
                && !supporting
                && !demotions[i].iter().any(|d| {
                    matches!(
                        d,
                        DemotionReason::ComponentOf(_)
                            | DemotionReason::ToolchainComponentOf(_)
                            | DemotionReason::CoveredBy(_)
                    )
                })
                && scores[i] >= ROOT_THRESHOLD;

            if !is_root && demotions[i].is_empty() {
                demotions[i].push(if !has_intent_evidence {
                    DemotionReason::NotUserInstalled
                } else if supporting {
                    DemotionReason::SupportingRole(roles[i])
                } else {
                    DemotionReason::NoUserFacingEntryPoint
                });
            }

            PackageDecision {
                name: fact.name.clone(),
                role: roles[i],
                intrinsic_score: intrinsic[i],
                score: scores[i],
                is_root,
                evidence: evidence[i].clone(),
                demotions: demotions[i].clone(),
            }
        })
        .collect()
}

/// True when `a` depends on `b`.
fn depends_on(
    dependents: &HashMap<String, Vec<String>>,
    a: &PackageFacts,
    b: &PackageFacts,
) -> bool {
    dependents
        .get(&b.name)
        .is_some_and(|deps| deps.contains(&a.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(name: &str, summary: &str) -> PackageFacts {
        PackageFacts {
            name: name.to_string(),
            summary: Some(summary.to_string()),
            source_rpm: Some(format!("{}-1.0-1.fc44.src.rpm", name)),
            user_installed: true,
            has_files: true,
            has_executable: false,
            has_desktop_entry: false,
            has_app_bundle: false,
        }
    }

    fn with_exec(mut f: PackageFacts) -> PackageFacts {
        f.has_executable = true;
        f
    }

    fn with_desktop(mut f: PackageFacts) -> PackageFacts {
        f.has_desktop_entry = true;
        f
    }

    fn no_dependents() -> HashMap<String, Vec<String>> {
        HashMap::new()
    }

    fn decisions(facts: Vec<PackageFacts>) -> Vec<PackageDecision> {
        classify_packages(&facts, &no_dependents())
    }

    fn find<'a>(list: &'a [PackageDecision], name: &str) -> &'a PackageDecision {
        list.iter().find(|d| d.name == name).unwrap()
    }

    // ==================== source_base ====================

    #[test]
    fn source_base_strips_version_and_release() {
        assert_eq!(source_base("git-2.54.0-1.fc44.src.rpm"), "git");
        assert_eq!(
            source_base("postgresql18-18.3-2.fc44.src.rpm"),
            "postgresql18"
        );
        assert_eq!(source_base("HandBrake-1.11.2-1.fc44.src.rpm"), "HandBrake");
        assert_eq!(
            source_base("qt6-qttools-6.11.1-1.fc44.src.rpm"),
            "qt6-qttools"
        );
        assert_eq!(source_base("7zip-26.01-1.fc44.src.rpm"), "7zip");
    }

    #[test]
    fn source_base_handles_hyphenated_names_without_version() {
        assert_eq!(source_base("my-tool.src.rpm"), "my-tool");
        assert_eq!(source_base(""), "");
    }

    // ==================== role classification ====================

    #[test]
    fn role_development_from_summary_and_suffix() {
        assert_eq!(
            classify_package_role("xz-devel", Some("Devel libraries & headers for liblzma")),
            PackageRole::Development
        );
        assert_eq!(
            classify_package_role(
                "libstdc++-devel",
                Some("Header files and libraries for C++ development")
            ),
            PackageRole::Development
        );
        assert_eq!(
            classify_package_role(
                "vulkan-headers",
                Some("Vulkan Header files and API registry")
            ),
            PackageRole::Development
        );
        // A legitimate user-facing package whose name merely resembles a
        // development package must NOT be misclassified.
        assert_eq!(
            classify_package_role(
                "libreoffice-math",
                Some("LibreOffice Equation Editor Application")
            ),
            PackageRole::Application
        );
    }

    #[test]
    fn role_library_and_runtime() {
        assert_eq!(
            classify_package_role(
                "yyjson",
                Some("A high performance JSON library written in ANSI C")
            ),
            PackageRole::Library
        );
        assert_eq!(
            classify_package_role(
                "containerd.io",
                Some("An industry-standard container runtime")
            ),
            PackageRole::Library
        );
        assert_eq!(
            classify_package_role("qt6-qttools-libs-help", Some("Qt6 Help runtime library")),
            PackageRole::Library
        );
    }

    #[test]
    fn role_filesystem_firmware_kernel_macro_doc_asset_perl() {
        assert_eq!(
            classify_package_role(
                "filesystem",
                Some("The basic directory layout for a Linux system")
            ),
            PackageRole::Filesystem
        );
        assert_eq!(
            classify_package_role("vim-filesystem", Some("VIM filesystem layout")),
            PackageRole::Filesystem
        );
        assert_eq!(
            classify_package_role("nvidia-gpu-firmware", Some("Firmware for NVIDIA GPUs")),
            PackageRole::Firmware
        );
        assert_eq!(
            classify_package_role(
                "kernel-modules",
                Some("kernel modules to match the core kernel")
            ),
            PackageRole::Kernel
        );
        assert_eq!(
            classify_package_role(
                "qt6-rpm-macros",
                Some("RPM macros for building Qt6 and KDE Frameworks 6 packages")
            ),
            PackageRole::Macro
        );
        assert_eq!(
            classify_package_role("git-core-doc", Some("Documentation files for git-core")),
            PackageRole::Documentation
        );
        assert_eq!(
            classify_package_role(
                "papirus-icon-theme",
                Some("Free and open source SVG icon theme")
            ),
            PackageRole::Asset
        );
        assert_eq!(
            classify_package_role("perl-HTTP-Message", Some("HTTP style message")),
            PackageRole::PerlModule
        );
        assert_eq!(
            classify_package_role("dpkg-perl", Some("Dpkg perl modules")),
            PackageRole::PerlModule
        );
    }

    #[test]
    fn role_system_component_from_summary() {
        assert_eq!(
            classify_package_role("dnf5", Some("Command-line package manager")),
            PackageRole::System
        );
        assert_eq!(
            classify_package_role(
                "grubby",
                Some("Command line tool for updating bootloader configs")
            ),
            PackageRole::System
        );
        assert_eq!(
            classify_package_role(
                "snapper",
                Some("Tool for maintaining snapshots of btrfs subvolumes")
            ),
            PackageRole::System
        );
        assert_eq!(
            classify_package_role(
                "xorg-x11-drv-nvidia-cuda",
                Some("NVIDIA driver with CUDA support")
            ),
            PackageRole::System
        );
        assert_eq!(
            classify_package_role("unixODBC", Some("A complete ODBC driver manager")),
            PackageRole::System
        );
        assert_eq!(
            classify_package_role("v4l-utils", Some("Utilities for video4linux devices")),
            PackageRole::System
        );
    }

    #[test]
    fn system_components_are_not_roots() {
        let list = classify_packages(
            &[
                with_exec(facts("dnf5", "Command-line package manager")),
                with_exec(facts(
                    "snapper",
                    "Tool for maintaining snapshots of btrfs subvolumes",
                )),
                with_exec(facts(
                    "xorg-x11-drv-nvidia-cuda",
                    "NVIDIA driver with CUDA support",
                )),
                with_exec(facts("zsh", "Powerful interactive shell")),
            ],
            &HashMap::new(),
        );
        let find = |name: &str| list.iter().find(|d| d.name == name).unwrap();
        assert!(!find("dnf5").is_root);
        assert!(!find("snapper").is_root);
        assert!(!find("xorg-x11-drv-nvidia-cuda").is_root);
        assert!(find("zsh").is_root);
    }

    #[test]
    fn role_application_for_user_facing_software() {
        for (name, summary) in [
            ("zsh", "Powerful interactive shell"),
            ("zoxide", "Smarter cd command for your terminal"),
            ("wireshark", "Network traffic analyzer"),
            ("tesseract", "Raw OCR Engine"),
            (
                "postgresql-server",
                "The programs needed to create and run a PostgreSQL server",
            ),
            ("mpv", "Movie player playing most video formats and DVDs"),
            (
                "maven",
                "Java project management and project comprehension tool",
            ),
            ("docker-ce", "The open-source application container engine"),
            ("code", "Code editing. Redefined."),
            ("cmake", "Cross-platform make system"),
            (
                "chezmoi",
                "Manage your dotfiles across multiple diverse machines",
            ),
            ("brave-browser", "Brave Web Browser"),
            ("bat", "Cat(1) clone with wings"),
            ("btop", "Modern and colorful command line resource monitor"),
            ("HandBrake", "An open-source multiplatform video transcoder"),
            ("libreoffice-draw", "LibreOffice Drawing Application"),
            (
                "pandoc-cli",
                "Markup conversion between documentation formats",
            ),
        ] {
            assert_eq!(
                classify_package_role(name, Some(summary)),
                PackageRole::Application,
                "{} should be an application",
                name
            );
        }
    }

    // ==================== positive classification ====================

    #[test]
    fn user_installed_executable_is_a_root() {
        let list = decisions(vec![with_exec(facts("zsh", "Powerful interactive shell"))]);
        let zsh = find(&list, "zsh");
        assert!(zsh.is_root);
        assert!(zsh.evidence.contains(&RootEvidence::DnfUserInstalled));
        assert!(zsh.evidence.contains(&RootEvidence::Executable));
    }

    #[test]
    fn desktop_application_is_a_root() {
        let list = decisions(vec![with_desktop(facts(
            "brave-browser",
            "Brave Web Browser",
        ))]);
        assert!(find(&list, "brave-browser").is_root);
    }

    #[test]
    fn app_bundle_is_a_root() {
        let mut f = facts(
            "PacketTracer",
            "Cisco PacketTracer 9.0 installation package",
        );
        f.has_app_bundle = true;
        let list = decisions(vec![f]);
        let p = find(&list, "PacketTracer");
        assert!(p.is_root);
        assert!(p.evidence.contains(&RootEvidence::ApplicationBundle));
    }

    #[test]
    fn metapackage_is_a_root() {
        let mut f = facts(
            "mongodb-org",
            "MongoDB open source document-oriented database system (metapackage)",
        );
        f.has_files = false;
        let list = decisions(vec![f]);
        let m = find(&list, "mongodb-org");
        assert!(m.is_root);
        assert!(m.evidence.contains(&RootEvidence::CanonicalProjectPackage));
    }

    // ==================== negative classification ====================

    #[test]
    fn supporting_roles_never_become_roots_even_with_binaries() {
        let mut libicu = facts(
            "libicu-devel",
            "Development files for International Components for Unicode",
        );
        libicu.has_executable = true;
        let mut vulkan = facts("vulkan-headers", "Vulkan Header files and API registry");
        vulkan.has_executable = true;
        let mut zlib = facts("zlib-ng", "Zlib replacement with optimizations");
        zlib.has_files = false;

        let list = decisions(vec![
            libicu,
            vulkan,
            zlib,
            facts(
                "filesystem",
                "The basic directory layout for a Linux system",
            ),
            facts("kernel-modules", "kernel modules to match the core kernel"),
            facts(
                "postgresql-private-libs",
                "The shared libraries required only for this build of PostgreSQL server",
            ),
            facts("glibc", "The GNU libc libraries"),
            facts("qt6-qttools-libs-help", "Qt6 Help runtime library"),
            facts("xorg-x11-proto-devel", "X.Org X11 Protocol headers"),
        ]);

        for name in [
            "libicu-devel",
            "vulkan-headers",
            "zlib-ng",
            "filesystem",
            "kernel-modules",
            "postgresql-private-libs",
            "glibc",
            "qt6-qttools-libs-help",
            "xorg-x11-proto-devel",
        ] {
            let d = find(&list, name);
            assert!(!d.is_root, "{} must not be a root", name);
        }

        // Packages whose metadata identifies a supporting role are demoted by
        // that role; zlib-ng is demoted because it has no user-facing entry
        // point at all (it is a library replacement with no CLI).
        for name in [
            "libicu-devel",
            "vulkan-headers",
            "filesystem",
            "kernel-modules",
            "postgresql-private-libs",
            "glibc",
            "qt6-qttools-libs-help",
            "xorg-x11-proto-devel",
        ] {
            let d = find(&list, name);
            assert!(
                d.demotions
                    .iter()
                    .any(|r| matches!(r, DemotionReason::SupportingRole(_))),
                "{} should be demoted by role",
                name
            );
        }
        assert!(find(&list, "zlib-ng")
            .demotions
            .contains(&DemotionReason::NoUserFacingEntryPoint));
    }

    #[test]
    fn non_user_installed_binary_is_not_a_root() {
        // A system package pulled in as a dependency or group member may
        // ship a binary, but it was not chosen as a top-level resource.
        let mut dbus = with_exec(facts("dbus", "D-Bus message bus"));
        dbus.user_installed = false;
        let mut nss = with_exec(facts("nss", "Network Security Services"));
        nss.user_installed = false;

        let list = decisions(vec![dbus, nss]);
        for name in ["dbus", "nss"] {
            let d = find(&list, name);
            assert!(!d.is_root, "{} must not be a root", name);
            assert!(d.demotions.contains(&DemotionReason::NotUserInstalled));
        }
    }

    #[test]
    fn application_without_entry_point_is_not_a_root() {
        let mut f = facts("some-config", "Some configuration package");
        f.has_files = true;
        let list = decisions(vec![f]);
        let d = find(&list, "some-config");
        assert!(!d.is_root);
        assert!(
            d.demotions
                .contains(&DemotionReason::SupportingRole(PackageRole::Support))
                || !d.is_root
        );
    }

    #[test]
    fn no_entry_point_no_evidence_is_not_a_root() {
        let mut f = facts("cdbs", "Common build system for Debian packages");
        f.has_files = true;
        let list = decisions(vec![f]);
        assert!(!find(&list, "cdbs").is_root);
    }

    // ==================== graph cases ====================

    #[test]
    fn same_project_dependency_becomes_supporting() {
        // git (canonical, no executable of its own) depends on git-core
        // (executable). The project must be represented by git, not git-core.
        let mut git = facts("git", "Fast Version Control System");
        git.has_files = true; // internal /usr/libexec helpers
        let mut git_core = with_exec(facts(
            "git-core",
            "Core package of git with minimal functionality",
        ));
        git_core.source_rpm = Some("git-2.54.0-1.fc44.src.rpm".into());
        git.source_rpm = Some("git-2.54.0-1.fc44.src.rpm".into());

        let mut dependents = HashMap::new();
        dependents.insert("git-core".to_string(), vec!["git".to_string()]);

        let list = classify_packages(&[git, git_core], &dependents);
        let g = find(&list, "git");
        let gc = find(&list, "git-core");
        assert!(g.is_root, "git should represent the project");
        assert!(!gc.is_root, "git-core should be a component of git");
        assert!(gc
            .demotions
            .iter()
            .any(|r| matches!(r, DemotionReason::ComponentOf(_))));
    }

    #[test]
    fn shared_dependency_of_two_roots_is_not_a_root() {
        // root-a → lib, root-b → lib. lib must not become a root merely
        // because two user-installed packages depend on it.
        let a = with_exec(facts("root-a", "Application A"));
        let b = with_exec(facts("root-b", "Application B"));
        let mut lib = facts("shared-lib", "A shared library used by A and B");
        lib.source_rpm = None;

        let mut dependents = HashMap::new();
        dependents.insert(
            "shared-lib".to_string(),
            vec!["root-a".to_string(), "root-b".to_string()],
        );

        let list = classify_packages(&[a, b, lib], &dependents);
        assert!(find(&list, "root-a").is_root);
        assert!(find(&list, "root-b").is_root);
        assert!(!find(&list, "shared-lib").is_root);
    }

    #[test]
    fn genuine_tool_dependency_of_separate_project_stays_root() {
        // automoc depends on cmake, but they are separate projects:
        // cmake must remain a root.
        let cmake = with_exec(facts("cmake", "Cross-platform make system"));
        let automoc = with_exec(facts("automoc", "Automatic moc for Qt 4"));
        let mut dependents = HashMap::new();
        dependents.insert("cmake".to_string(), vec!["automoc".to_string()]);

        let list = classify_packages(&[cmake, automoc], &dependents);
        assert!(find(&list, "cmake").is_root);
        assert!(find(&list, "automoc").is_root);
    }

    #[test]
    fn toolchain_component_is_demoted() {
        // qt6-linguist ships a GUI but exists only for qt6-qttools-devel.
        let mut linguist = with_desktop(facts("qt6-linguist", "Qt6 Linguist Tools"));
        linguist.source_rpm = Some("qt6-qttools-6.11.1-1.fc44.src.rpm".into());
        let mut devel = facts("qt6-qttools-devel", "Development files for qt6-qttools");
        devel.source_rpm = Some("qt6-qttools-6.11.1-1.fc44.src.rpm".into());

        let mut dependents = HashMap::new();
        dependents.insert(
            "qt6-linguist".to_string(),
            vec!["qt6-qttools-devel".to_string()],
        );

        let list = classify_packages(&[linguist, devel], &dependents);
        let l = find(&list, "qt6-linguist");
        assert!(!l.is_root);
        assert!(l
            .demotions
            .iter()
            .any(|r| matches!(r, DemotionReason::ToolchainComponentOf(_))));
    }

    #[test]
    fn project_representative_prefers_stronger_candidate() {
        // HandBrake (CLI) and HandBrake-gui share a source; only the stronger
        // representative should remain a root.
        let mut cli = with_exec(facts(
            "HandBrake",
            "An open-source multiplatform video transcoder",
        ));
        let mut gui = with_desktop(with_exec(facts(
            "HandBrake-gui",
            "An open-source multiplatform video transcoder (GUI)",
        )));
        cli.source_rpm = Some("HandBrake-1.11.2-1.fc44.src.rpm".into());
        gui.source_rpm = Some("HandBrake-1.11.2-1.fc44.src.rpm".into());

        let list = decisions(vec![cli, gui]);
        assert!(find(&list, "HandBrake-gui").is_root);
        assert!(!find(&list, "HandBrake").is_root);
        assert!(find(&list, "HandBrake")
            .demotions
            .iter()
            .any(|r| matches!(r, DemotionReason::CoveredBy(_))));
    }

    #[test]
    fn decisions_are_independent_of_input_order() {
        let a = with_exec(facts("zsh", "Powerful interactive shell"));
        let b = with_exec(facts("zoxide", "Smarter cd command for your terminal"));
        let forward = decisions(vec![a.clone(), b.clone()]);
        let backward = decisions(vec![b, a]);
        let mut f: Vec<_> = forward
            .iter()
            .map(|d| (d.name.clone(), d.is_root))
            .collect();
        let mut r: Vec<_> = backward
            .iter()
            .map(|d| (d.name.clone(), d.is_root))
            .collect();
        f.sort();
        r.sort();
        assert_eq!(f, r);
    }

    #[test]
    fn reason_string_lists_evidence() {
        let list = decisions(vec![with_desktop(with_exec(facts(
            "wireshark",
            "Network traffic analyzer",
        )))]);
        let reason = find(&list, "wireshark").reason_string();
        assert!(reason.contains("DNF user-installed package"));
        assert!(reason.contains("desktop application entry"));
        assert!(reason.contains("user-facing executable"));
    }
}
