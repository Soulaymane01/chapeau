use chapeau::backends::dnf::DnfCliBackend;
use chapeau::backends::package_backend::PackageBackend;

fn dnf5_available() -> bool {
    DnfCliBackend::new().is_available()
}

#[test]
fn test_dnf5_is_available() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    assert!(backend.is_available());
}

#[test]
fn test_discover_installed_packages() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    let packages = backend
        .discover_installed()
        .expect("discover_installed failed");

    assert!(
        !packages.is_empty(),
        "should have at least one installed package"
    );

    // Every package should have non-empty name and version
    for p in &packages {
        assert!(!p.name.is_empty(), "package name must not be empty");
        assert!(!p.version.is_empty(), "package version must not be empty");
        assert!(!p.arch.is_empty(), "package arch must not be empty");
        assert!(
            !p.repository.is_empty(),
            "package repository must not be empty"
        );
    }

    // Should contain bash (essential system package)
    let has_bash = packages.iter().any(|p| p.name == "bash");
    assert!(has_bash, "installed packages should include bash");
}

#[test]
fn test_discover_repositories() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    let repos = backend
        .discover_repositories()
        .expect("discover_repositories failed");

    assert!(!repos.is_empty(), "should have at least one repository");

    for r in &repos {
        assert!(!r.id.is_empty(), "repo id must not be empty");
        assert!(!r.name.is_empty(), "repo name must not be empty");
    }

    // Should have the fedora repo
    let has_fedora = repos.iter().any(|r| r.id == "fedora");
    assert!(has_fedora, "repositories should include fedora");
}

#[test]
fn test_query_dependencies_bash() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    let deps = backend
        .query_dependencies("bash")
        .expect("query_dependencies failed");

    // bash should have at least one dependency
    assert!(!deps.is_empty(), "bash should have dependencies");

    for d in &deps {
        assert_eq!(d.source_package, "bash");
        assert!(!d.dependency_string.is_empty());
    }

    // bash depends on filesystem
    let has_filesystem = deps
        .iter()
        .any(|d| d.dependency_string.contains("filesystem"));
    assert!(has_filesystem, "bash should depend on filesystem");
}

#[test]
fn test_query_reverse_dependencies_bash() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    let rdeps = backend
        .query_reverse_dependencies("bash")
        .expect("query_reverse_dependencies failed");

    // bash should have reverse dependencies
    assert!(!rdeps.is_empty(), "bash should have reverse dependencies");

    // All returned names should be non-empty
    for name in &rdeps {
        assert!(
            !name.is_empty(),
            "reverse dependency name must not be empty"
        );
    }
}

#[test]
fn test_nonexistent_package_no_deps() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    let deps = backend
        .query_dependencies("this-package-definitely-does-not-exist-xyz123")
        .expect("query_dependencies failed for nonexistent package");
    assert!(
        deps.is_empty(),
        "nonexistent package should have no dependencies"
    );
}

#[test]
fn test_parse_live_repoquery_output() {
    if !dnf5_available() {
        eprintln!("dnf5 not found — skipping integration test");
        return;
    }
    let backend = DnfCliBackend::new();
    let packages = backend
        .discover_installed()
        .expect("discover_installed failed");

    // The discovered packages should be parseable into our types
    // and have sensible values
    let total = packages.len();
    let user = packages
        .iter()
        .filter(|p| p.reason == chapeau::discovery::packages::InstallReason::User)
        .count();
    let dep = packages
        .iter()
        .filter(|p| p.reason == chapeau::discovery::packages::InstallReason::Dependency)
        .count();

    // At least some packages should be dependencies
    assert!(
        dep > 0,
        "there should be at least some dependency packages (total: {}, user: {}, dep: {})",
        total,
        user,
        dep
    );
}
