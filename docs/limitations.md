# Chapeau Limitations

This document describes what Chapeau does NOT currently do. These are current limitations, not a roadmap.

## What Chapeau is NOT

Chapeau is not:

- **A package manager.** Chapeau does not install, update, or remove packages on its own. It delegates to DNF5.
- **A replacement for DNF.** Chapeau does not solve dependencies, resolve conflicts, or manage repositories.
- **A replacement for systemd.** Chapeau does not start, stop, enable, or disable services.
- **A Nix/NixOS clone.** Chapeau does not provide reproducible system specifications, lockfiles, or declarative configuration.
- **An Ansible replacement.** Chapeau does not provision systems, run playbooks, or manage configurations.
- **A project environment manager.** Chapeau does not manage virtual environments, containers, or project-specific dependencies.
- **A daemon.** Chapeau does not run in the background or monitor the system continuously.

## Current limitations

### No mutation of Flatpak applications

Chapeau discovers Flatpak applications and runtimes but cannot install, update, or remove them. Use the `flatpak` CLI for Flatpak operations.

### No mutation of systemd services

Chapeau discovers systemd services and their states but cannot start, stop, enable, or disable them. Use `systemctl` for service operations.

### No repository management

Chapeau records which repositories exist and how many packages come from each, but cannot add, remove, enable, or disable repositories. Use `dnf config-manager` for repository management.

### No configuration file tracking

Chapeau does not track configuration files, their contents, or their relationships to packages.

### No hardware relationship modeling

Chapeau does not model relationships between hardware devices and system resources.

### No firewall or network modeling

Chapeau does not model firewall rules, network configurations, or network dependencies.

### No automatic cleanup

Chapeau does not automatically remove orphaned or unused resources. Analysis commands (`orphaned`, `unused`) identify candidates but do not act on them.

### No rollback

If a removal causes problems, Chapeau cannot undo it. Use DNF's history or system backups for rollback.

### No reproducible system specification

Chapeau cannot export a "this is my system" specification that can be applied to another machine.

### GUI parity

The GTK4/libadwaita frontend (`chapeau-gui`) covers the current feature set:
My System overview, resource detail, Explore/search, status and drift
dashboards, domain management, orphaned/unused analysis, scanning, removal
via pkexec, and the Graphviz dependency graph view. Packaging is not
finished: the RPM spec is a starting point that has not been built, and the
AppStream metadata lacks a homepage URL. See [Chapeau GUI](gui.md).

### No background monitoring

Chapeau does not run as a daemon or service. It only queries the system when you run a command.

### No real-time state tracking

Chapeau's database reflects the system state at the time of the last scan. Changes made between scans are not reflected in the model until the next scan, although `chapeau drift` can show them (read-only) on demand.

### No service dependency analysis

Chapeau discovers services and their states but does not analyze systemd dependency graphs (Wants, Requires, After, Before).

### No Flatpak-to-package relationships

Chapeau does not model relationships between Flatpak applications and system packages (e.g., a Flatpak app that uses a system library).

### No cross-resource dependency analysis

Chapeau tracks `depends_on` relationships within the same resource type (e.g., package-to-package) but does not analyze cross-type dependencies (e.g., "which packages does this service need?").

### Limited domain analysis

Domain analysis currently checks ownership and usage but does not perform deep cross-domain dependency analysis. The `find_other_domain_dependencies` function merges owning and using domains but does not build a full cross-domain graph.

### Drift detection is a comparison, not a history

`chapeau drift` compares the recorded model against a live discovery. It reports the current differences (missing, new, changed); it does not keep a time-series change log. Per-scan drift summaries are recorded in `history`.

### Single-database architecture

Chapeau uses a single SQLite database per user. There is no multi-user, multi-system, or remote system support.

## What is implemented but limited

### Removal scope

`chapeau remove` currently only handles RPM packages via DNF5. It does not remove Flatpak applications, systemd services, or repositories.

### Analysis depth

`orphaned` and `unused` analysis uses heuristics based on relationships and domain assignments. These are not proof that a resource is safe to remove. A resource marked as unused may still be needed by untracked system components.

### Provenance tracking

Provenance fields are populated during scan but not all fields may be filled for all resource types. The depth of provenance information depends on what the backends report.

## Future directions (not implemented)

These are areas that could be explored in future phases but are NOT currently implemented:

- Flatpak mutation (install/remove/update)
- Service management (start/stop/enable/disable)
- Repository management
- Configuration file tracking
- Reproducible system specification
- Multi-system support
- Full GUI parity (explore, domains, removal, graph)
- Background monitoring
- Automatic cleanup suggestions with risk assessment
