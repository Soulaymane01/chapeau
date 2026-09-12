# Changelog

## 0.1.0 — 2026-09-12

First release: Chapeau as a semantic layer over Fedora, with a terminal and a
graphical frontend sharing one model.

### Terminal (`chapeau`)

- **Scan and reconcile**: DNF packages, repositories, the package dependency
  graph, systemd units (including installed-but-not-loaded ones), and Flatpak
  applications, runtimes and remotes, in one atomic transaction.
- **Intentional resources (roots)**: evidence-based classification using DNF
  install reasons, package metadata, roles, entry points, project structure
  and the dependency graph; explicit roots always win, detected roots are
  recomputed per scan.
- **Progressive disclosure**: `chapeau` (My System overview), `chapeau show`,
  `chapeau roots`, and `--all` views for packages, services and Flatpaks.
- **Queries and explanation**: `why`, `dependencies`, `dependents`, `status`,
  `drift`, `graph` (Graphviz DOT), `repositories`.
- **Organization**: user-defined domains (`domains create/add/remove/delete`).
- **Safety**: `orphaned`/`unused` analysis with explicit heuristics caveats,
  removal impact plans, `remove` with confirmation, post-removal
  reconciliation, and package-manager authority preserved.
- **Provenance**: install reasons, source RPMs, install dates, repository
  origins, and a full action history.

### Desktop (`chapeau-gui`, GTK4 + libadwaita)

- My System home with resources grouped by domain or type.
- Resource detail with facts, intent, domains, dependencies/providers, and
  install date.
- Explore views (packages, services, Flatpaks, repositories) with
  intentional-by-default listings and All switches for power users.
- Services grouped by state with Start/Stop and Enable/Disable via pkexec.
- Status and drift dashboards, domain management, orphaned/unused analysis,
  dependency graph rendering, and impact-preview package removal via pkexec.
- Desktop entry, AppStream metadata and scalable icon.

### Known limitations

- Fedora-only backends (DNF5, systemd, Flatpak).
- The RPM spec is a starting point and has not been built in mock/koji.
- AppStream metadata has no homepage URL yet.
- Service start/stop is GUI-only; the CLI lists but does not control services.
- GUI views are validated manually; semantics are covered by shared-service
  tests.
