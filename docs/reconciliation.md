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

- **Drift detection** — Comparing Chapeau's model against the live system without a full scan (stub exists in `reconciliation/drift.rs`)
- **Automatic background reconciliation** — No daemon or periodic reconciliation
- **Selective reconciliation** — Reconciling only specific resource types (except packages via `reconcile_after_removal`)
- **Service reconciliation** — After removing a package, systemd service state is not automatically re-queried
- **Flatpak reconciliation** — After Flatpak operations, Chapeau state is not automatically updated

For now, `chapeau scan` is the primary way to resynchronize Chapeau's state with reality after unexpected changes.
