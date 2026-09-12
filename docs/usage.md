# Chapeau Usage Guide

This guide walks through a typical workflow for using Chapeau on a Fedora system.

## Typical workflow

```
1. Scan the system
2. Inspect system status
3. Explore the My System overview and resource details
4. Ask why something exists
5. Organize resources into domains
6. Look for potentially orphaned or unused resources
7. Preview a removal
8. Confirm removal
9. Reconcile the resulting system state
```

Each step is explained below with actual commands.

## Step 1: Scan the system

```bash
chapeau scan
```

This discovers everything currently installed on your Fedora system:

- **Packages** from DNF5 (installed RPMs with installation reason and origin repository)
- **Repositories** configured in DNF5
- **Systemd services** with their active/enabled/failed state
- **Flatpak applications** and runtimes with their remotes

The scan stores all discovered information in Chapeau's SQLite database. Running scan again is safe and idempotent — it updates existing records and adds new ones without creating duplicates.

## Step 2: Check system status

```bash
chapeau status
```

Shows a summary of what Chapeau knows:

- Total resources (packages, services, flatpaks, repositories)
- Number of domains
- Number of relationships
- Services breakdown (active, enabled, failed)
- Repository list
- Observation count

## Step 3: Explore the system

### My System overview

```bash
chapeau
chapeau overview
```

Shows the default "My System" view: intentional resources grouped by domain
when you have created domains, otherwise grouped by resource type, with
counts of tracked resources and relationships. Missing intentional resources
are flagged. See [My System](my-system.md).

### Show a resource

```bash
chapeau show <resource>
```

Prints everything Chapeau knows about one resource in context: type-specific
facts, version and state, provenance, root intent, domain memberships, and its
immediate dependencies/dependents.

### List intentional resources

```bash
chapeau roots
```

Shows the semantic list of software Chapeau classifies as intentionally
present: user-facing applications, tools, servers and Flatpak apps.
Explicit declarations made with `chapeau root add` are shown as `[user]`;
automatically classified resources are shown as `[detected]`.

Detection uses package metadata and the dependency graph, not DNF's
user-installed flag alone. Supporting packages (development headers,
libraries, firmware, Perl modules, filesystem layouts) are tracked but not
shown here. See [Intentional Resources (Roots)](roots.md).

### List packages

```bash
chapeau packages
chapeau packages --all
```

`chapeau packages` lists intentional packages only. `chapeau packages --all`
discovers the full installed package set from DNF5 and shows the breakdown
by installation reason (user, dependency, group), the repository summary,
and the user-installed packages.

### List services

```bash
chapeau services
chapeau services --all
```

`chapeau services` lists intentional services (none by default — services
are not auto-detected as roots). `chapeau services --all` shows all systemd
services grouped by state: active, failed, inactive.

### List repositories

```bash
chapeau repositories
```

Shows DNF repositories known to Chapeau with the count of packages from each.

### List Flatpak applications

```bash
chapeau flatpaks
chapeau flatpaks --all
```

`chapeau flatpaks` lists intentional Flatpak applications; installed
applications are strong root candidates. `chapeau flatpaks --all` shows all
installed Flatpak applications, runtimes, and configured remotes. Requires
Flatpak to be installed.

### Check backend availability

```bash
chapeau backends
```

Shows whether DNF5, systemd, and Flatpak are available on your system.

## Step 4: Understand relationships

### Why is something installed?

```bash
chapeau why <resource-name>
```

For example:

```bash
chapeau why bash
```

This explains:
- Whether the resource is an intentional root and why it was detected
- The package's semantic role (application, development package, library...)
  and DNF install reason
- The package's current status (installed version, active state)
- Where it came from (origin repository)
- What depends on it
- What it depends on
- Which domains own or use it

### Show dependencies

```bash
chapeau dependencies <resource-name>
```

Shows what a resource depends on (outgoing `depends_on` relationships).

### Show reverse dependencies

```bash
chapeau dependents <resource-name>
```

Shows what depends on a resource (incoming `depends_on` relationships).

## Step 5: Organize into domains

Domains are logical groupings that help you understand why resources exist on your system. They do not affect how resources behave.

### Create a domain

```bash
chapeau domains create development -d "Development tools and libraries"
```

### Add resources to a domain

```bash
chapeau domains add development gcc
chapeau domains add development gdb
chapeau domains add development python3 --owns -r "Core language for development"
```

The `--owns` flag means the domain has authoritative ownership. The default is `--uses`.

### List domains

```bash
chapeau domains list
```

Shows all domains with their resources.

### Remove a resource from a domain

```bash
chapeau domains remove development gdb
```

## Step 6: Find potentially orphaned or unused resources

### Find orphaned resources

```bash
chapeau orphaned
```

Shows resources that have:
- No remaining dependents
- No domain ownership
- No domain usage

These are candidates for review, but **not automatically safe to remove**.

### Find unused resources

```bash
chapeau unused
```

Shows resources that meet all five unused criteria:
- No dependents
- No domain ownership
- No domain usage
- No outgoing dependencies
- No interactions with other resources

These are stronger candidates for review, but still require human judgment.

**Important:** Neither `orphaned` nor `unused` removes anything. They are analysis tools only.

## Step 7: Preview a removal

Before removing anything, preview what would happen:

```bash
chapeau remove <resource-name>
```

Chapeau first shows a removal plan with:
- The resource to be removed
- Affected services
- Packages that would be removed
- Packages that are still required by other things
- Domain ownership and usage
- Warnings

This is a preview only. You are prompted to confirm before anything happens.

## Step 8: Confirm removal

If the plan looks correct, confirm the removal. Chapeau will:

1. Execute the actual removal via DNF5 (`sudo dnf5 remove -y <package>`)
2. Reconcile Chapeau's database with the new system state
3. Clean up stale records (observations, relationships, domain associations)

If DNF5 fails, Chapeau does not modify its database.

If reconciliation fails after a successful DNF removal, Chapeau logs the error but the DNF removal itself is still in effect. Run `chapeau scan` to fully resynchronize.

## Step 9: Resynchronize after unexpected changes

If you make changes to your system outside of Chapeau (manual DNF operations, system updates, etc.):

```bash
# See exactly what differs (read-only); does not change the database
chapeau drift

# Re-discover the current system state and update the database to match
chapeau scan
```

`chapeau drift` reports missing, new, and changed resources. `chapeau scan`
then reconciles them: new resources are adopted, changed observations are
updated, and resources no longer present are marked missing while their
records (and any explicit root state) are preserved.

If an explicit root's software disappeared, `chapeau roots` shows it as
`[missing]` until it is reinstalled or its root state is removed. See
[Intentional Resources (Roots)](roots.md#missing-roots).

## Safety principles

- **Chapeau never removes anything without explicit user confirmation.**
- **Analysis results (`orphaned`, `unused`) are heuristics, not proof that something is safe to delete.**
- **Fedora is always the authority on what exists. Chapeau records intent and meaning.**
- **Scanning is idempotent. Running it multiple times is safe.**
