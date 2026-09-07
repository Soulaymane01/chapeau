# Chapeau Removal

## Overview

`chapeau remove` is the only command that modifies the live system. It removes a package via DNF5 and then reconciles Chapeau's database.

**Chapeau does not implement its own dependency solver.** It delegates the actual removal to DNF5, which handles RPM dependency resolution.

## Removal workflow

```
chapeau remove <resource>
      ↓
1. Look up the resource in the database
      ↓
2. Build a removal plan (analysis only)
      ↓
3. Display the plan and prompt for confirmation
      ↓
4. On confirmation: run sudo dnf5 remove -y <package>
      ↓
5. On success: reconcile Chapeau's database
      ↓
6. Record the operation in history
```

## What the removal plan shows

Before any system change, Chapeau displays a plan:

- **Resource to remove** — The package being targeted
- **Affected services** — Systemd services that originate from this package
- **Packages to remove** — The target package (and anything DNF5 identifies)
- **Packages still required** — Other packages that depend on this one (DNF5's dependency analysis)
- **Domain ownership** — Domains that own this resource
- **Domain usage** — Domains that use this resource
- **Warnings** — Anything the user should be aware of

Example plan:

```
Removal Plan: vim

Resource: vim (package)
  Version: 9.1.0

Affected services:
  (none)

Packages to remove:
  vim

Packages still required by other packages:
  (none)

Domain ownership:
  (none)

Domain usage:
  (none)

Warnings:
  (none)
```

## Confirmation

Chapeau prompts:

```
Continue? [y/N]
```

Only `y` or `Y` proceeds. Any other input (including just pressing Enter) cancels the operation.

## What happens on confirmation

### 1. DNF5 removal

Chapeau runs:

```bash
sudo dnf5 remove -y <package>
```

This is a real system change. The package and any packages DNF5 identifies as removable are removed from the system.

### 2. Post-removal reconciliation

After a successful DNF5 removal, Chapeau reconciles its database:

1. Re-discovers installed packages from DNF5
2. Compares against Package resources in the database
3. Deletes stale packages (ones no longer installed)
4. Foreign key cascades clean up associated observations, relationships, domain associations, and provenance
5. Records the operation in history

### 3. History recording

The removal is recorded in the history table with:
- Action: `remove`
- Resource type and ID
- Details about what was removed
- Timestamp

## Failure scenarios

### DNF5 fails

If DNF5 cannot remove the package (dependency conflict, permission denied, package not found):

1. Chapeau prints the error
2. **Chapeau does NOT modify its database**
3. The operation is recorded as a failure in history
4. The system remains unchanged

### DNF5 succeeds but reconciliation fails

If DNF5 removes the package successfully but Chapeau's reconciliation fails:

1. The package IS removed from the system (DNF5 committed the change)
2. Chapeau's database becomes stale — the removed package still appears
3. Chapeau prints a warning about the reconciliation failure
4. The user can run `chapeau scan` to resynchronize

This is a deliberate design choice: the system state is always the truth, even if Chapeau's database is temporarily out of sync.

### Resource not found

If the resource name does not match any package in the database:

1. Chapeau prints a `ResourceNotFound` error
2. No system changes are made

## Safety principles

- **Chapeau never removes anything without explicit user confirmation.**
- **Chapeau does not remove a resource merely because it appears unused.** The `orphaned` and `unused` commands are analysis tools, not removal triggers.
- **Chapeau does not implement its own dependency solver.** DNF5 handles RPM dependency resolution.
- **If DNF5 fails, nothing changes.** Chapeau does not perform partial removals.
- **System state is always the authority.** Chapeau's database is updated to match reality, not the other way around.

## What Chapeau does NOT remove

Currently, `chapeau remove` only handles RPM packages via DNF5. It does not remove:

- Flatpak applications
- systemd services directly
- Repository configurations
- Files outside of DNF5's management
- User data or configuration files

These are all handled by their respective native tools.
