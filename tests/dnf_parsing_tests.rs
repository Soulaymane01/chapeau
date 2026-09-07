use chapeau::backends::dnf::{
    parse_nevra_name, parse_repo_list_json, parse_repoquery_output, parse_requires_output,
    parse_whatdepends_output,
};
use chapeau::discovery::packages::{DependencyType, InstallReason};

// ==================== Package Record parsing ====================

#[test]
fn parse_repoquery_single_package() {
    let input = "bash\t5.3.9-3.fc44\tx86_64\tfedora\tDependency\tfedora\t1782268815\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "bash");
    assert_eq!(pkgs[0].version, "5.3.9-3.fc44");
    assert_eq!(pkgs[0].arch, "x86_64");
    assert_eq!(pkgs[0].repository, "fedora");
    assert_eq!(pkgs[0].reason, InstallReason::Dependency);
    assert_eq!(pkgs[0].from_repo.as_deref(), Some("fedora"));
    assert_eq!(pkgs[0].install_time, Some(1782268815));
}

#[test]
fn parse_repoquery_user_installed() {
    let input = "HandBrake\t1.11.2-1.fc44\tx86_64\trpmfusion-free-updates\tUser\trpmfusion-free-updates\t1783430494\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "HandBrake");
    assert_eq!(pkgs[0].reason, InstallReason::User);
}

#[test]
fn parse_repoquery_group_installed() {
    let input = "firefox\t133.0-1.fc44\tx86_64\tfedora\tGroup\tfedora\t1782268815\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].reason, InstallReason::Group);
}

#[test]
fn parse_repoquery_multiple_packages() {
    let input = "\
bash\t5.3.9-3.fc44\tx86_64\tfedora\tDependency\tfedora\t1782268815
vim-enhanced\t9.1.0814-1.fc44\tx86_64\tupdates\tUser\tupdates\t1787779218
python3\t3.13.5-2.fc44\tx86_64\tfedora\tDependency\tfedora\t1782268815
";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 3);
    assert_eq!(pkgs[0].name, "bash");
    assert_eq!(pkgs[1].name, "vim-enhanced");
    assert_eq!(pkgs[2].name, "python3");
}

#[test]
fn parse_repoquery_empty_input() {
    let pkgs = parse_repoquery_output("");
    assert!(pkgs.is_empty());
}

#[test]
fn parse_repoquery_blank_lines() {
    let input = "\n\n\n";
    let pkgs = parse_repoquery_output(input);
    assert!(pkgs.is_empty());
}

#[test]
fn parse_repoquery_malformed_line_too_few_fields() {
    let input = "only_name\n";
    let pkgs = parse_repoquery_output(input);
    assert!(pkgs.is_empty());
}

#[test]
fn parse_repoquery_missing_optional_fields() {
    // Only 4 fields (no reason, from_repo, installtime)
    let input = "bash\t5.3.9-3.fc44\tx86_64\tfedora\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].reason, InstallReason::Unknown);
    assert!(pkgs[0].from_repo.is_none());
    assert!(pkgs[0].install_time.is_none());
}

#[test]
fn parse_repoquery_invalid_install_time() {
    let input = "bash\t5.3.9-3.fc44\tx86_64\tfedora\tDependency\tfedora\tnot_a_number\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert!(pkgs[0].install_time.is_none());
}

#[test]
fn parse_repoquery_unknown_reason() {
    let input = "bash\t5.3.9-3.fc44\tx86_64\tfedora\tSomethingElse\t\t\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].reason, InstallReason::Unknown);
}

#[test]
fn parse_repoquery_with_epoch() {
    let input = "ImageMagick\t1:7.1.2.13-2.fc44\tx86_64\tfedora\tDependency\tfedora\t1783429922\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "ImageMagick");
    assert_eq!(pkgs[0].version, "1:7.1.2.13-2.fc44");
}

#[test]
fn parse_repoquery_noarch() {
    let input = "filesystem\t3.18-4.fc44\tnoarch\tfedora\tDependency\tfedora\t1782268815\n";
    let pkgs = parse_repoquery_output(input);
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].arch, "noarch");
}

// ==================== Repository list JSON parsing ====================

#[test]
fn parse_repo_list_single() {
    let json = r#"[{"id":"fedora","name":"Fedora 44","is_enabled":true}]"#;
    let repos = parse_repo_list_json(json).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].id, "fedora");
    assert_eq!(repos[0].name, "Fedora 44");
    assert!(repos[0].is_enabled);
}

#[test]
fn parse_repo_list_multiple() {
    let json = r#"[
        {"id":"fedora","name":"Fedora 44","is_enabled":true},
        {"id":"updates","name":"Fedora 44 Updates","is_enabled":true},
        {"id":"rpmfusion","name":"RPM Fusion","is_enabled":false}
    ]"#;
    let repos = parse_repo_list_json(json).unwrap();
    assert_eq!(repos.len(), 3);
    assert!(repos[0].is_enabled);
    assert!(repos[1].is_enabled);
    assert!(!repos[2].is_enabled);
}

#[test]
fn parse_repo_list_empty() {
    let json = "[]";
    let repos = parse_repo_list_json(json).unwrap();
    assert!(repos.is_empty());
}

#[test]
fn parse_repo_list_invalid_json() {
    let result = parse_repo_list_json("not json");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid repo list JSON"));
}

#[test]
fn parse_repo_list_missing_fields() {
    // Missing "name" field
    let json = r#"[{"id":"fedora","is_enabled":true}]"#;
    let result = parse_repo_list_json(json);
    assert!(result.is_err());
}

// ==================== Requires output parsing ====================

#[test]
fn parse_requires_multiple_deps() {
    let input = "/usr/bin/sh
config(bash) = 5.3.9-3.fc44
filesystem >= 3
libc.so.6()(64bit)
";
    let deps = parse_requires_output("bash", input);
    assert_eq!(deps.len(), 4);
    assert_eq!(deps[0].source_package, "bash");
    assert_eq!(deps[0].dependency_string, "/usr/bin/sh");
    assert_eq!(deps[0].dep_type, DependencyType::Requires);
    assert_eq!(deps[1].dependency_string, "config(bash) = 5.3.9-3.fc44");
    assert_eq!(deps[2].dependency_string, "filesystem >= 3");
    assert_eq!(deps[3].dependency_string, "libc.so.6()(64bit)");
}

#[test]
fn parse_requires_empty() {
    let deps = parse_requires_output("bash", "");
    assert!(deps.is_empty());
}

#[test]
fn parse_requires_single_dep() {
    let input = "libc.so.6()(64bit)\n";
    let deps = parse_requires_output("coreutils", input);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].source_package, "coreutils");
}

// ==================== Whatdepends output parsing ====================

#[test]
fn parse_whatdepends_nevra() {
    let input = "\
ModemManager-0:1.24.2-3.fc44.x86_64
NetworkManager-1:1.56.1-2.fc44.x86_64
PackageKit-0:1.3.4-3.fc44.x86_64
";
    let names = parse_whatdepends_output(input);
    assert_eq!(names.len(), 3);
    assert_eq!(names[0], "ModemManager");
    assert_eq!(names[1], "NetworkManager");
    assert_eq!(names[2], "PackageKit");
}

#[test]
fn parse_whatdepends_simple_nevra() {
    let input = "vim-enhanced-9.1.0814-1.fc44.x86_64\n";
    let names = parse_whatdepends_output(input);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "vim-enhanced");
}

#[test]
fn parse_whatdepends_empty() {
    let names = parse_whatdepends_output("");
    assert!(names.is_empty());
}

#[test]
fn parse_whatdepends_noarch() {
    let input = "bash-completion-3.1.8-2.fc44.noarch\n";
    let names = parse_whatdepends_output(input);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "bash-completion");
}

// ==================== NEVRA name parsing ====================

#[test]
fn nevra_name_simple() {
    assert_eq!(parse_nevra_name("bash-5.3.9-3.fc44.x86_64"), "bash");
}

#[test]
fn nevra_name_with_epoch() {
    assert_eq!(
        parse_nevra_name("ImageMagick-1:7.1.2.13-2.fc44.x86_64"),
        "ImageMagick"
    );
}

#[test]
fn nevra_name_noarch() {
    assert_eq!(
        parse_nevra_name("filesystem-3.18-4.fc44.noarch"),
        "filesystem"
    );
}

#[test]
fn nevra_name_hyphenated() {
    assert_eq!(
        parse_nevra_name("NetworkManager-l2tp-0:1.52.2-1.fc44.x86_64"),
        "NetworkManager-l2tp"
    );
}

#[test]
fn nevra_name_i686() {
    assert_eq!(parse_nevra_name("glibc-2.40-3.fc44.i686"), "glibc");
}

#[test]
fn nevra_name_no_suffix_fallback() {
    // No arch suffix — should return as-is
    assert_eq!(parse_nevra_name("some-thing"), "some-thing");
}

// ==================== InstallReason ====================

#[test]
fn install_reason_from_dnf5() {
    assert_eq!(
        InstallReason::from_dnf5("Dependency"),
        InstallReason::Dependency
    );
    assert_eq!(InstallReason::from_dnf5("User"), InstallReason::User);
    assert_eq!(InstallReason::from_dnf5("Group"), InstallReason::Group);
    assert_eq!(InstallReason::from_dnf5("unknown"), InstallReason::Unknown);
    assert_eq!(InstallReason::from_dnf5(""), InstallReason::Unknown);
}

#[test]
fn install_reason_as_str() {
    assert_eq!(InstallReason::Dependency.as_str(), "Dependency");
    assert_eq!(InstallReason::User.as_str(), "User");
    assert_eq!(InstallReason::Group.as_str(), "Group");
    assert_eq!(InstallReason::Unknown.as_str(), "Unknown");
}

#[test]
fn install_reason_display() {
    assert_eq!(InstallReason::User.to_string(), "User");
}

// ==================== Serialization round-trips ====================

#[test]
fn package_record_serialization() {
    use chapeau::discovery::packages::PackageRecord;
    let p = PackageRecord {
        name: "bash".into(),
        version: "5.3.9-3.fc44".into(),
        arch: "x86_64".into(),
        repository: "fedora".into(),
        reason: InstallReason::Dependency,
        from_repo: Some("fedora".into()),
        install_time: Some(1782268815),
    };
    let json = serde_json::to_string(&p).unwrap();
    let back: PackageRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(p, back);
}

#[test]
fn repo_record_serialization() {
    use chapeau::discovery::packages::RepoRecord;
    let r = RepoRecord {
        id: "fedora".into(),
        name: "Fedora 44".into(),
        is_enabled: true,
    };
    let json = serde_json::to_string(&r).unwrap();
    let back: RepoRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(r, back);
}

#[test]
fn dependency_record_serialization() {
    use chapeau::discovery::packages::DependencyRecord;
    let d = DependencyRecord {
        source_package: "bash".into(),
        dependency_string: "libc.so.6()(64bit)".into(),
        dep_type: DependencyType::Requires,
    };
    let json = serde_json::to_string(&d).unwrap();
    let back: DependencyRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(d, back);
}

// ==================== SystemSnapshot ====================

#[test]
fn system_snapshot_empty() {
    use chapeau::discovery::packages::SystemSnapshot;
    let snap = SystemSnapshot::empty();
    assert_eq!(snap.package_count(), 0);
    assert_eq!(snap.user_installed_count(), 0);
    assert_eq!(snap.dependency_count(), 0);
    assert!(snap.find_package("bash").is_none());
    assert!(snap.packages_from_repo("fedora").is_empty());
    assert!(snap.enabled_repos().is_empty());
}

#[test]
fn system_snapshot_counts() {
    use chapeau::discovery::packages::{PackageRecord, RepoRecord, SystemSnapshot};
    let snap = SystemSnapshot {
        packages: vec![
            PackageRecord {
                name: "bash".into(),
                version: "5.3.9-3.fc44".into(),
                arch: "x86_64".into(),
                repository: "fedora".into(),
                reason: InstallReason::Dependency,
                from_repo: None,
                install_time: None,
            },
            PackageRecord {
                name: "vim".into(),
                version: "9.1.0814-1.fc44".into(),
                arch: "x86_64".into(),
                repository: "updates".into(),
                reason: InstallReason::User,
                from_repo: None,
                install_time: None,
            },
        ],
        repositories: vec![
            RepoRecord {
                id: "fedora".into(),
                name: "Fedora 44".into(),
                is_enabled: true,
            },
            RepoRecord {
                id: "updates".into(),
                name: "Fedora 44 Updates".into(),
                is_enabled: true,
            },
        ],
        snapshot_time: "2025-01-01T00:00:00Z".into(),
    };

    assert_eq!(snap.package_count(), 2);
    assert_eq!(snap.user_installed_count(), 1);
    assert_eq!(snap.dependency_count(), 1);
    assert!(snap.find_package("bash").is_some());
    assert!(snap.find_package("nonexistent").is_none());
    assert_eq!(snap.packages_from_repo("fedora").len(), 1);
    assert_eq!(snap.packages_from_repo("updates").len(), 1);
    assert_eq!(snap.enabled_repos().len(), 2);
}
