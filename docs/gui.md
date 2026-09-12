# Chapeau GUI

Chapeau has two frontends over one shared model:

```text
chapeau          terminal interface (default)
chapeau-gui      graphical interface (GTK4 + libadwaita)
```

Both use the same `chapeau::services` layer and the same SQLite database, so
they can never disagree about semantics. The CLI keeps working exactly as
before.

## Status

The GUI is an **early read-only preview**:

| View | Status |
|------|--------|
| My System overview (roots grouped by domain/type) | Implemented |
| Resource detail (facts, intent, domains, dependencies) | Implemented |
| Explore/search packages, services, Flatpaks, repositories | Implemented |
| Flatpaks: intentional apps by default, All switch | Implemented |
| Status dashboard | Implemented |
| Drift dashboard with reconcile | Implemented |
| Domain management (create, delete, memberships) | Implemented |
| Scan with live progress | Implemented |
| Services by state with start/stop and enable/disable (pkexec) | Implemented |
| Removal (impact preview + pkexec) | Implemented |
| Dependency graph view (Graphviz) | Implemented |
| Orphaned / unused analysis | Implemented |
| Hide/show resources in My System | Implemented |

## Building

The GUI needs the GTK development headers in addition to the normal
prerequisites:

```bash
sudo dnf install gtk4-devel libadwaita-devel
cargo build -p chapeau-gui
```

`cargo build` at the repository root still builds the CLI only; the GUI is a
separate workspace member (`gui/`) so the CLI and its tests stay free of GTK
dependencies.

## Running

```bash
# Open the My System overview
cargo run -p chapeau-gui

# Open directly at one resource's detail page
cargo run -p chapeau-gui -- postgresql-server
```

The GUI uses the same database as the CLI
(`~/.local/state/chapeau/chapeau.db`). If Chapeau has never scanned, the
overview says so and the **Scan** button runs a full scan with live progress.

## Installing the desktop entry

For a user-local install (no root needed):

```bash
install -Dm755 target/release/chapeau      ~/.local/bin/chapeau
install -Dm755 target/release/chapeau-gui  ~/.local/bin/chapeau-gui
install -Dm644 gui/resources/org.chapeau.Chapeau.desktop \
    ~/.local/share/applications/org.chapeau.Chapeau.desktop
install -Dm644 gui/resources/org.chapeau.Chapeau.metainfo.xml \
    ~/.local/share/metainfo/org.chapeau.Chapeau.metainfo.xml
install -Dm644 gui/resources/icons/hicolor/scalable/apps/org.chapeau.Chapeau.svg \
    ~/.local/share/icons/hicolor/scalable/apps/org.chapeau.Chapeau.svg

update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

Chapeau then appears in the application menu as **Chapeau**
(`~/.local/bin` must be in `PATH`).

The metadata files are validated in-tree with:

```bash
desktop-file-validate gui/resources/org.chapeau.Chapeau.desktop
appstreamcli validate --no-net gui/resources/org.chapeau.Chapeau.metainfo.xml
```

`packaging/chapeau.spec` is an RPM spec starting point: it parses
(`rpmspec -P`) and installs both binaries plus the desktop/metainfo/icon
files, but it has not been built in mock/koji yet and needs the Fedora
vendored-crates cargo macros before submission.

## Architecture

```text
GTK main loop (gui/src/main.rs, gui/src/ui/)
      │  Request                     Response
      ▼                                ▲
Worker thread (gui/src/service.rs) ────┘
      │ owns the Database
      ▼
chapeau::services (overview, detail, scan, drift, …)
      │
      ▼
chapeau core/storage (SQLite, graph, analysis)
```

- The GTK main thread never touches SQLite. Every query is a `Request` sent
  to a worker thread that owns the database connection.
- Results return as `Response` values delivered on the GTK main context
  through an async channel (`glib::spawn_future_local`).
- A scan reports `ScanEvent`s (backend phases, dependency resolution,
  commit), which the UI shows as progress. The window stays responsive
  during long DNF queries.
- The GUI only consumes `chapeau::services` view models: no SQL, no backend
  calls, no semantic decisions in the UI.

## Relationship to the CLI

`chapeau` and `chapeau-gui` are two presentations of the same model. For
example:

| CLI | GUI |
|-----|-----|
| `chapeau` / `chapeau overview` | My System sidebar + home page |
| `chapeau show <resource>` | Resource detail page |
| `chapeau scan` | Scan button with progress |

Anything the GUI shows can be verified against the terminal, and vice versa.

## Navigation

The sidebar is navigation only, and the lists live in the main content area:

```text
My System    intentional resources (grouped by domain or type)
System       Status · Drift · Services · Domains · Orphaned · Unused
Explore      Packages · Services · Flatpaks · Repositories
```

**System** and **Explore** expand when clicked; selecting an entry pushes the
corresponding view. This keeps the sidebar short regardless of how many
resources you have; the full lists render as the main content.

## Domains

Domains are user-defined organization, never automatic. In the GUI:

- **Menu → Domains** lists every domain with its memberships. Create with
  the **+** button, delete with the trash button (memberships are removed;
  resources are preserved), and remove individual memberships.
- The resource detail page's **Domains** group shows current memberships and
  offers **Add to domain** with an owns/uses choice.

All mutations go through `services::domains` and are recorded in history, so
the CLI (`chapeau domains …`) and GUI stay in lockstep.

## Graph

Every resource detail page has **Show dependency graph**. The GUI builds the
same Graphviz neighborhood as `chapeau graph` (`services::graph`, depth 2,
capped at 200 nodes), renders it to PNG with the `dot` binary on the worker
thread, and shows it in a scrollable view. Graphviz (`graphviz` package) is
therefore a runtime dependency for this view; without it the GUI reports a
clear error. The CLI keeps its composable form:
`chapeau graph <resource> | dot -Tsvg > graph.svg`.

## Removal

A package's detail page has a destructive **Remove package…** action:

1. The impact plan is built read-only (`services::removal::plan`, the same
   `RemovalPlan` the CLI shows) and presented in a dialog.
2. On confirmation the removal runs through the package backend via
   **pkexec**, so polkit prompts graphically. Package-manager knowledge
   stays in the backend (`removal_argv`); only the privilege launcher
   differs between CLI (`sudo`) and GUI (`pkexec`).
3. If the OS command fails, Chapeau's model is untouched and the error is
   shown. If it succeeds, the post-removal reconciliation runs; a
   reconciliation failure is reported as "run Scan", matching the CLI's
   behaviour.

## Services

**Menu → System → Services** lists systemd units grouped by state — Failed,
Running, Stopped. By default it shows only *user* services: services that are
intentional roots or that are provided by a package you deliberately have
(e.g. `postgresql.service` from `postgresql-server`, `mongod.service` from
the MongoDB project, `docker.service` from `docker-ce`). The **All services**
switch reveals every installed unit (750+ on a typical Fedora install),
tagged `system` when they are not user services. Stopped services have a
**Start** button, running ones a **Stop** button; the second button toggles
boot persistence (**Enable**/**Disable**). All actions go through systemctl
with a graphical privilege prompt, and the unit's recorded state is refreshed
afterwards.

Service attribution comes from package file lists: discovery records which
systemd unit files each package ships (`services::units`). Unit files owned by
a subpackage are attributed to the project's deliberate root package, so a
`mongodb-org-server` unit still belongs to `mongodb-org`.

## Hiding resources

Detected roots are Chapeau's best guess, not your statement. From a resource
detail page, **Hide from My System** keeps it fully tracked (Explore, graph,
drift) but out of the default sidebar; **Show in My System** restores it.
Hidden resources are preserved across scans and never re-detected. The CLI
equivalents are `chapeau root hide <resource>` / `root unhide <resource>`
and `chapeau roots --all` lists hidden entries.

## Analysis

**Menu → System → Orphaned / Unused** opens the cleanup analysis with a
dropdown to switch modes. Each entry is an expander showing the reasons
(no dependents, no domain ownership/usage, no dependencies) plus still-
required-by lists, domain involvement and affected services. This mirrors
`chapeau orphaned` and `chapeau unused`, including the caveat banner: these
are heuristics, and nothing is removed automatically. Long lists are capped
at 100 entries in the GUI; the CLI prints the complete set.

## Packaging status

- Desktop entry, AppStream metainfo and a scalable icon live in
  `gui/resources/`.
- `desktop-file-validate` passes. `appstreamcli validate` passes except for
  `url-homepage-missing`, which stays until the project has a public URL.
- `packaging/chapeau.spec` parses but is not built yet (see above).

## Status and Drift

The header menu also opens:

- **Status** — resource counts, domains, untracked/missing counts,
  relationships and observation totals, service health, and the repository
  list. A refresh button reloads it.
- **Drift** — Missing / New / Changed sections from a live read-only
  comparison, with a **Reconcile** button that runs a full scan and refreshes
  the report. The scan itself reports progress in the home page.

Both are the same `services::status` and `services::drift` models the CLI
uses (`chapeau status`, `chapeau drift`).

## Explore

Explore opens searchable, database-backed views for packages, services and
repositories. Searching filters by name, version, origin and native id; rows
tagged `[root]` or `[missing]` match the model. Selecting a row opens the
resource detail page.

Flatpaks has its own view, matching how the sidebar used to present them:
only **intentional Flatpak applications** appear by default, and the
**All Flatpaks** switch reveals every installed application and runtime
(grouped as Applications and Runtimes). Like everything else in the GUI,
Explore reads the recorded model — use **Scan** to refresh it.
