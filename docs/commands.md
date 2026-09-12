# Chapeau Command Reference

All commands are run as `chapeau <command>`. Running `chapeau` with no command
shows the My System overview. Use `chapeau --help` for a list of available
commands.

A graphical frontend (`chapeau-gui`) exposes the same model; see
[Chapeau GUI](gui.md).

## overview

**Purpose:** Show the My System overview: intentional resources grouped by
domain (when configured) or by resource type.

```
chapeau
chapeau overview
```

**Arguments:** None

**Options:** None

**Behavior:** Lists roots with their source tags, counts intentional
resources, tracked resources and relationships, and points at the next
commands. The ungrouped list is truncated when domains exist; `chapeau roots`
lists every root. See [My System](my-system.md).

---

## scan

**Purpose:** Discover the full system state and update Chapeau's database.

```
chapeau scan
```

**Arguments:** None

**Options:** None

**Behavior:**
1. Discovers installed packages from DNF5
2. Discovers configured repositories from DNF5
3. Discovers systemd services via D-Bus
4. Discovers Flatpak applications and runtimes (if Flatpak is available)
5. For each discovered resource, creates or updates the database record
6. Records observations (current state) for each resource
7. Creates `comes_from` relationships between packages and their origin repositories
8. Creates `uses` relationships between Flatpak apps and their runtimes
9. Computes drift against the pre-scan state and records it in history
10. Marks resources no longer reported by a complete backend as currently
    missing (`installed = false`), preserving the resource and its root state

**Example output:**
```
Discovered system state:
  Packages:       1423
  Repositories:   8
  Services:       187
  Flatpak apps:   12
  Flatpak runtimes: 5
  Flatpak remotes: 3
```

If something changed since the last scan, a compact drift summary follows:

```
Drift detected since the last scan (1 new, 2 changed):
...
```

---

## status

**Purpose:** Show a summary of Chapeau's knowledge about the system.

```
chapeau status
```

**Arguments:** None

**Options:** None

**Behavior:** Reads all resources, domains, relationships, and observations from the database and prints a summary, including the number of resources recorded as missing.

---

## show

**Purpose:** Show detailed information about one resource.

```
chapeau show <RESOURCE>
```

**Arguments:**
- `<RESOURCE>` — native resource ID (package name, unit name, Flatpak ID, or repository)

**Options:** None

**Behavior:** Prints the resource's type-specific facts (package version,
service state, Flatpak branch, repository package count), provenance,
intent/root status, domain memberships, and its immediate dependency
relationships. Missing intentional resources are flagged with guidance.
Resolution prefers package, then Flatpak, then service when a name is
ambiguous. See [My System](my-system.md#resource-detail).

---

## graph

**Purpose:** Render the dependency neighborhood of a resource as Graphviz DOT.

```
chapeau graph <RESOURCE> [--depth N]
```

**Arguments:**
- `<RESOURCE>` — native resource ID

**Options:**
- `--depth N` — how many dependency hops to follow (default 2)

**Behavior:** Walks `depends_on` edges from the resource, capped at 200
nodes, and prints DOT to stdout (a summary goes to stderr). Pipe it to
Graphviz:

```bash
chapeau graph postgresql-server --depth 2 | dot -Tsvg > postgresql.svg
```

The GUI opens the same neighborhood as a rendered PNG from a resource's
detail page.

---

## drift

**Purpose:** Show what differs between Chapeau's recorded state and the live system, without changing anything.

```
chapeau drift
```

**Arguments:** None

**Options:** None

**Behavior:** Performs a live discovery (DNF5, systemd, Flatpak) and compares
it against the database. Reports three kinds of change:

- **Missing** — recorded as present, not found on the live system
- **New** — found on the live system but not previously recorded
- **Changed** — still present, with a different version or service state

The command is read-only. Resources already recorded as absent are not
reported again, and "New" entries require an existing baseline of that
resource type. Run `chapeau scan` to reconcile what drift found.

**Example output:**
```
Discovering current system state (read-only)...

System Drift (1 missing, 2 changed)

Missing (1):
  postgresql-server    package   last seen 16.4-1.fc44

Changed (2):
  firefox              package   128.0-1.fc44 -> 129.0-1.fc44
  sshd.service         service   active yes -> active no

Run 'chapeau scan' to reconcile Chapeau's knowledge with the current system.
```

See [reconciliation.md](reconciliation.md#drift-detection).

---

## roots

**Purpose:** List and manage intentional resources (roots).

```
chapeau roots
chapeau roots list
chapeau roots add <RESOURCE> [--reason <TEXT>]
chapeau roots remove <RESOURCE>
```

**Subcommands:**
- `list` (default) — list roots grouped by resource type, with source
  (`[user]` or `[detected]`) and `[missing]` for roots whose resource is no
  longer present
- `add` — explicitly declare a resource as an intentional root
- `remove` — remove root state only; never uninstalls or deletes

**Behavior:** `add` always wins over automatic classification: a detected
root is promoted to source `user` and preserved across scans. `remove`
deletes only the semantic root state; a removed detected root may be
re-detected by a later scan because detection is recomputed from evidence.
Explicit roots whose resource disappears stay as missing.
See [Intentional Resources (Roots)](roots.md).

---

## packages

**Purpose:** List packages.

```
chapeau packages [--all]
```

**Arguments:** None

**Options:**
- `--all` — discover and show the complete installed package set

**Behavior:** By default, lists intentional packages (roots) from the
Chapeau database. With `--all`, uses the DNF5 backend to discover installed
packages and shows:
- Total installed packages by reason (user, dependency, group)
- Repository summary with package counts
- Top 20 user-installed packages (name, version, architecture, origin repository)

---

## services

**Purpose:** List systemd services tracked by Chapeau.

```
chapeau services [--all]
```

**Arguments:** None

**Options:**
- `--all` — show all services instead of intentional services only

**Behavior:** By default, lists intentional services (services are not
auto-detected as roots). With `--all`, lists services from the Chapeau
database grouped by state (active, failed, inactive). Requires a prior
`chapeau scan` to populate data.

---

## flatpaks

**Purpose:** List Flatpak applications.

```
chapeau flatpaks [--all]
```

**Arguments:** None

**Options:**
- `--all` — show all applications and runtimes

**Behavior:** By default, lists intentional Flatpak applications from the
Chapeau database. Installed Flatpak applications are strong root
candidates; runtimes are not roots. With `--all`, uses the Flatpak backend
to discover installed applications, runtimes, and remotes. Requires Flatpak
to be installed on the system.

---

## repositories

**Purpose:** List DNF repositories tracked by Chapeau.

```
chapeau repositories
```

**Arguments:** None

**Options:** None

**Behavior:** Lists repositories from the Chapeau database with the count of packages from each repository. Requires a prior `chapeau scan`.

---

## why

**Purpose:** Explain why a resource is installed and how it relates to other resources.

```
chapeau why <RESOURCE>
```

**Arguments:**
- `RESOURCE` (required) — Resource name (package name, service name, etc.)

**Options:** None

**Behavior:** Looks up the resource by name and builds an explanation showing:
- Root status: source and the evidence recorded when it was detected
- Package role (application, development package, library, ...) and DNF install reason
- Current status (installed version, active/enabled/failed state)
- Origin (where it came from)
- What depends on it
- What it depends on
- Domain ownership and usage
- What it provides

---

## dependencies

**Purpose:** Show what a resource depends on.

```
chapeau dependencies <RESOURCE>
```

**Arguments:**
- `RESOURCE` (required) — Resource name

**Options:** None

**Behavior:** Shows all outgoing `depends_on` relationships from the named resource.

---

## dependents

**Purpose:** Show what depends on a resource.

```
chapeau dependents <RESOURCE>
```

**Arguments:**
- `RESOURCE` (required) — Resource name

**Options:** None

**Behavior:** Shows all incoming `depends_on` relationships to the named resource.

---

## domains

**Purpose:** List or manage logical resource groupings.

```
chapeau domains [SUBCOMMAND]
```

**Subcommands:**

### domains list

```
chapeau domains list
```

Lists all domains with their resources and the relationship type (owns/uses).

### domains create

```
chapeau domains create <NAME> [-d <DESCRIPTION>]
```

**Arguments:**
- `NAME` (required) — Domain name

**Options:**
- `-d, --description` — Optional description

Creates a new domain. If the domain already exists, prints a message and exits.

### domains delete

```
chapeau domains delete <NAME>
```

**Arguments:**
- `NAME` (required) — Domain name

Deletes a domain and its memberships. Resources themselves are never touched.

### domains add

```
chapeau domains add <DOMAIN> <RESOURCE> [--owns] [--uses] [-r <REASON>]
```

**Arguments:**
- `DOMAIN` (required) — Domain name
- `RESOURCE` (required) — Resource name (native_id)

**Options:**
- `--owns` — Use the `owns` relationship (authoritative ownership)
- `--uses` — Use the `uses` relationship (default if neither flag is set)
- `-r, --reason` — Optional reason for this relationship

Adds a resource to a domain. If the relationship already exists, prints a message and exits.

### domains remove

```
chapeau domains remove <DOMAIN> <RESOURCE>
```

**Arguments:**
- `DOMAIN` (required) — Domain name
- `RESOURCE` (required) — Resource name (native_id)

Removes the relationship between a domain and a resource. Removes both `owns` and `uses` relationships if they exist.

---

## orphaned

**Purpose:** Analyze potentially orphaned resources.

```
chapeau orphaned
```

**Arguments:** None

**Options:** None

**Behavior:** Identifies resources that have no remaining dependents, no domain ownership, and no domain usage. This is analysis only — nothing is removed.

---

## unused

**Purpose:** Analyze potentially unused resources.

```
chapeau unused
```

**Arguments:** None

**Options:** None

**Behavior:** Identifies resources that meet all five unused criteria: no dependents, no domain ownership, no domain usage, no outgoing dependencies, and no interactions. This is analysis only — nothing is removed.

---

## remove

**Purpose:** Plan and execute removal of a resource.

```
chapeau remove <RESOURCE>
```

**Arguments:**
- `RESOURCE` (required) — Resource name (native_id)

**Options:** None

**Behavior:**
1. Builds a removal plan showing affected resources, services, domain associations, and warnings
2. Displays the plan and prompts for confirmation
3. On confirmation, executes removal via DNF5 (`sudo dnf5 remove -y`)
4. After successful removal, reconciles Chapeau's database
5. Records the operation in history

**Warning:** This command runs `sudo dnf5 remove`. It performs a real system change. Always review the plan before confirming.

---

## backends

**Purpose:** Show which system backends are available.

```
chapeau backends
```

**Arguments:** None

**Options:** None

**Behavior:** Checks for the presence of DNF5, systemd D-Bus, and Flatpak on the system and reports their availability.

---

## db

**Purpose:** Database management and diagnostics.

```
chapeau db <SUBCOMMAND>
```

### db status

```
chapeau db status
```

Shows database diagnostics:
- Database file path
- WAL mode status
- Foreign key enforcement status
- Resource, relationship, and domain counts
