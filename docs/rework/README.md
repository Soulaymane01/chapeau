# Chapeau Inventory Manager Rework

This directory is the human-facing index for the rework from the legacy
semantic-layer implementation to the inventory/control-panel product.

## Current state

- **Legacy implementation archived at:** `legacy-v0.1.0-semantic` (commit `582688e`)
- **Rework branch:** `rework/inventory-v2`
- **Detailed plan:** [`docs/superpowers/plans/2026-11-05-chapeau-inventory-rework.md`](../superpowers/plans/2026-11-05-chapeau-inventory-rework.md)
- **Legacy git bundle:** `~/.local/state/chapeau/backups/chapeau-legacy-20261105-162905.bundle`
- **Legacy database backup:** `~/.local/state/chapeau/backups/chapeau-pre-rework-20261105-162905.db`

## Product definition

Chapeau is a Fedora-native inventory and control panel:

- **Apps** — launchable software from desktop entries, Flatpak, and manual entries.
- **Services** — systemd services and user-defined custom command services.
- **Groups** — user-defined collections such as IDEs, Databases, and Web.
- **Facts** — state, ports, URLs, package, unit path, working directory, logs.

It is not a dependency analyzer, package manager, Nix clone, or daemon.

## Phase map

| Phase | Scope | Status |
|---|---|---|
| 0 | Archive legacy state, create branch, back up database, write plan | Complete |
| 1 | New model, schema v4, storage | Complete |
| 2 | App/service/port/custom-service discovery and scan | Next |
| 3 | Catalog views and CLI read commands | Pending |
| 4 | App launch, service control, group bulk actions | Pending |
| 5 | GUI dashboard, apps, services, groups, detail, settings | Pending |
| 6 | Delete legacy semantic core and migrate old tables | Pending |
| 7 | Docs, packaging, full QA, release | Pending |

### Phase 1 commits

```text
94d5f2a feat: add inventory model types
bd8754c feat: add inventory schema v4
b17a77e feat: add item and group storage
24c5a42 feat: add inventory status, ports, custom services, and scan storage
3e314b2 test: verify legacy database migrates to inventory schema
```

Phase 1 delivered:

- `src/model/` — Item, Group, AppStatus, ServiceStatus, ServicePort, CustomServiceDefinition
- schema v4 — items, groups, group_items, app_status, service_status, service_ports, custom_service_definitions, settings, scan_runs
- storage CRUD for items, groups, status, ports, custom services, scan runs, settings
- verified migration of the real legacy database backup to v4 while preserving old tables

### Phase 0 artifacts

- Tag: `legacy-v0.1.0-semantic`
- Branch: `rework/inventory-v2`
- Feature branch: `feat/inventory-foundation`
- Git bundle: `~/.local/state/chapeau/backups/chapeau-legacy-20261105-162905.bundle`
- Database backup: `~/.local/state/chapeau/backups/chapeau-pre-rework-20261105-162905.db`

## Decisions locked in

- Apps default to desktop entries + Flatpak + manual entries.
- RPM package inventory is secondary, not the default app list.
- Services support systemd and custom commands.
- Custom commands run as the current user only, never through `sudo`/`pkexec`.
- Groups support Start all / Stop all / Restart all in configured order.
- Package uninstall/removal is deferred out of v1.
- No dependency graph, roots, orphan analysis, or removal planner in the new core.

## Rollback

```bash
git switch master
git reset --hard legacy-v0.1.0-semantic

cp ~/.local/state/chapeau/backups/chapeau-pre-rework-20261105-162905.db \
   ~/.local/state/chapeau/chapeau.db
rm -f ~/.local/state/chapeau/chapeau.db-wal \
      ~/.local/state/chapeau/chapeau.db-shm
```

Do not delete the `legacy-v0.1.0-semantic` tag until the rework has shipped
and the bundle/database backup is no longer needed.
