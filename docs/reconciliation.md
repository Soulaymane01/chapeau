# Chapeau Reconciliation

## Why reconciliation exists

Fedora is the authoritative source for what exists on the system. Chapeau observes Fedora and records what it reports. But Fedora can change independently of Chapeau:

- The user might install or remove packages directly via DNF
- System updates might add or remove packages
- Flatpak operations might change application state

Without reconciliation, Chapeau's database would drift from reality. Reconciliation is the process of detecting and correcting that drift.

```
Fedora reality
     ↓
Discovery (backends query live system)
     ↓
SystemSnapshot (what actually exists now)
     ↓
Reconciliation (compare against stored state)
     ↓
Chapeau model (corrected to match reality)
```

## Full scan reconciliation

The primary reconciliation mechanism is `chapeau scan`:

```bash
chapeau scan
```

This performs a full discovery of the system state and reconciles it against the database:

1. **Discover** — Query DNF5, systemd, and Flatpak for the current system state
2. **Compare** — For each discovered resource, check if it already exists in the database
3. **Upsert** — Create new resources, update existing ones
4. **Observe** — Update or create observation records for each resource
5. **Relate** — Create or verify relationships (package-to-repo, app-to-runtime)

All of this happens within a single SQLite transaction. If any step fails, the entire operation is rolled back.

Scan is idempotent — running it multiple times produces the same result as running it once.

## Post-removal reconciliation

When `chapeau remove` successfully removes a package via DNF5, Chapeau performs targeted reconciliation:

```bash
chapeau remove <resource>
  ↓
DNF5 removes the package
  ↓
reconcile_after_removal()
```

This reconciliation:

1. **Re-discovers** installed packages from DNF5 (not the full system, just packages)
2. **Compares** the discovered package list against all Package resources in the database
3. **Identifies** stale packages — resources in the database that are no longer installed
4. **Deletes** stale packages from the database
5. **Cleans up** associated data via foreign key cascades:
   - Observations for removed packages
   - Relationships involving removed packages
   - Domain-resource associations for removed packages
   - Provenance records for removed packages
6. **Records** the operation in history

This is implemented in `reconciliation::scanner::reconcile_after_removal()`.

### ReconcileSummary

Post-removal reconciliation returns a summary:

```
ReconcileSummary {
    stale_packages_removed: 1,
    observations_cleaned: 1,
    relationships_cleaned: 3,
    domain_associations_cleaned: 1,
}
```

If no stale packages are found:

```
No stale resources found.
```

If stale packages are cleaned:

```
Reconciled 1 stale package(s).
  Observations cleaned: 1
  Relationships cleaned: 3
  Domain associations cleaned: 1
  Associated observations, relationships, and domain associations cleaned automatically.
```

## Drift detection

Drift is the difference between what Chapeau has recorded and what the live
system currently reports. Two mechanisms surface it:

### `chapeau drift` (read-only)

```bash
chapeau drift
```

This performs a live discovery **without writing anything** and compares it
against the recorded model:

```text
System Drift (1 missing, 1 new, 2 changed)

Missing (1):
  postgresql-server    package   last seen 16.4-1.fc44

New (1):
  neovim               package   0.10.2-1.fc44

Changed (2):
  firefox              package   128.0-1.fc44 -> 129.0-1.fc44
  sshd.service         service   active yes -> active no

Run 'chapeau scan' to reconcile Chapeau's knowledge with the current system.
```

Drift is a **change report**:

- Resources already recorded as absent are not reported again.
- "New" entries require a baseline: on a database with no recorded resources
  of a type, freshly discovered resources are adoptions, not drift.
- Only resources observed as present are reported missing. Absence is only
  inferred when the owning backend was queried successfully.
- Service units that appear are not listed as "New" (transient units would
  dominate); service state changes and vanished units are reported.

`chapeau drift` requires an existing baseline. On a fresh database it tells
you to run `chapeau scan` first.

### Drift during `chapeau scan`

A scan computes the same report against the pre-scan state before it writes
anything, records it in `history` (`drift.detect`) when non-empty, and prints
a compact summary:

```text
Drift detected since the last scan (1 missing, 2 changed):
...
Marked 1 resource(s) as currently missing (recorded state preserved).
```

### Missing marking

After a complete discovery run, resources that were previously observed but
are no longer reported are marked absent:

1. **Only complete backends may declare absence.** A backend that is not
   available, or a discovery that failed, contributes nothing. This preserves
   the invariant that partial discovery is never interpreted as loss.
2. **Only the observation changes.** `installed` becomes `false`; the
   resource, its relationships, provenance, domain associations and root
   state are all preserved.
3. **Roots keep their semantics.** Explicit roots are shown as missing
   instead of being dropped; detected roots are recomputed away because
   their evidence came from the live system.
4. **Reappearance clears the flag.** The next scan writes a fresh observation
   with `installed = true`.

## Reconciliation failure modes

### Full scan failure

If `chapeau scan` fails (backend unavailable, database error), the transaction is rolled back and no changes are made. The previous database state is preserved.

### Post-removal reconciliation failure

If the DNF5 removal succeeds but reconciliation fails:

1. The DNF5 removal is **not rolled back** — the package is genuinely removed from the system
2. Chapeau's database becomes stale — the removed package still appears in the database
3. Chapeau prints a warning about the reconciliation failure
4. The user can run `chapeau scan` to fully resynchronize

This is a deliberate design choice: the system state is always correct, even if Chapeau's database is temporarily out of sync.

## What is NOT yet implemented

The following reconciliation mechanisms are not currently implemented:

- **Automatic background reconciliation** — No daemon or periodic reconciliation
- **Selective reconciliation** — Reconciling only specific resource types (except packages via `reconcile_after_removal`)
- **Service reconciliation after removal** — After removing a package, systemd service state is not automatically re-queried by `reconcile_after_removal` (a full `chapeau scan` does reconcile services)
- **Flatpak reconciliation after mutation** — After Flatpak operations, Chapeau state is not automatically updated

For now, `chapeau scan` is the primary way to resynchronize Chapeau's state
with reality after unexpected changes, and `chapeau drift` shows what differs
without changing anything.
