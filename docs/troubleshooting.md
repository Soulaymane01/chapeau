# Chapeau Troubleshooting

## Database issues

### Database location

Chapeau's database is at:

```
~/.local/state/chapeau/chapeau.db
```

If the directory does not exist, running `chapeau scan` creates it automatically.

### Permission problems

If you see errors about database access:

```bash
# Check ownership
ls -la ~/.local/state/chapeau/

# Fix permissions if needed
chmod 755 ~/.local/state/chapeau
chmod 644 ~/.local/state/chapeau/chapeau.db
```

Chapeau runs as your user. The database should be owned by your user account.

### Corrupt database

If the database is corrupted (rare, but possible after a system crash):

1. Back up the existing database:
   ```bash
   cp ~/.local/state/chapeau/chapeau.db ~/.local/state/chapeau/chapeau.db.bak
   ```

2. Delete the corrupted database:
   ```bash
   rm ~/.local/state/chapeau/chapeau.db
   ```

3. Rebuild from scratch:
   ```bash
   chapeau scan
   ```

This loses all domain assignments and user-created relationships, but the system state is rediscovered from Fedora.

### Migration issues

Chapeau automatically applies database migrations on startup. If migration fails:

1. Check the error message for details
2. Ensure the database file is not locked by another process
3. If the issue persists, delete the database and rescan (see above)

### SQLite locking

Chapeau uses WAL mode with a 5-second busy timeout. If you see "database is locked" errors:

- Ensure only one Chapeau process is running at a time
- Do not manually edit the database while Chapeau is running

## DNF5 issues

### DNF5 not available

If `chapeau backends` shows DNF5 as "not found":

1. Install DNF5:
   ```bash
   sudo dnf install dnf5
   ```

2. Verify it works:
   ```bash
   dnf5 --version
   ```

### Permission denied

Commands like `chapeau remove` run `sudo dnf5 remove`. If sudo is required:

1. Ensure your user has sudo privileges
2. You may be prompted for your password during removal

### Package not found

If `chapeau remove <name>` reports the resource is not found:

1. Run `chapeau scan` to update the database
2. Check the exact package name with `chapeau packages`
3. The resource must exist in Chapeau's database, not just on the system

### Malformed DNF5 output

If DNF5 output cannot be parsed:

1. Check that DNF5 is the version expected (DNF5, not DNF4)
2. Run `chapeau backends` to verify DNF5 is available
3. Report the issue with the exact command output

## systemd issues

### D-Bus connection problems

If systemd services are not discovered:

1. Check that systemd is running:
   ```bash
   systemctl status
   ```

2. Check D-Bus connectivity:
   ```bash
   busctl status
   ```

3. If running in a container, systemd and D-Bus may not be available. Chapeau will still work for packages and Flatpaks.

### Permission denied for service operations

Service discovery uses the system D-Bus, which requires appropriate permissions. If running as a regular user, most read operations work. If you see permission errors, try running with elevated privileges.

## Flatpak issues

### Flatpak not installed

If `chapeau backends` shows Flatpak as "not found":

1. Install Flatpak:
   ```bash
   sudo dnf install flatpak
   ```

2. Add Flathub (if needed):
   ```bash
   flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
   ```

3. Re-run the scan:
   ```bash
   chapeau scan
   ```

### No Flatpak applications

If Flatpak is installed but no applications appear:

- Flatpak applications are installed per-user or system-wide
- Check with `flatpak list --app`
- Chapeau discovers both user and system installations

## Reconciliation issues

### Scan failure

If `chapeau scan` fails:

1. Check backend availability: `chapeau backends`
2. Check database status: `chapeau db status`
3. Check for errors in the output
4. The scan is transactional — if it fails, the database is unchanged

### Stale state after system changes

If you install or remove packages outside of Chapeau:

```bash
chapeau scan
```

This re-discovers the current system state and updates the database.

### Post-removal reconciliation failure

If `chapeau remove` succeeds but reconciliation fails:

1. The package IS removed from the system (DNF committed the change)
2. Chapeau's database is stale
3. Run `chapeau scan` to resynchronize

## Recovery

The primary recovery mechanism is:

```bash
chapeau scan
```

This re-discovers the entire system state and updates the database to match. It is safe to run at any time.

If the database is completely broken, delete it and rescan:

```bash
rm ~/.local/state/chapeau/chapeau.db
chapeau scan
```

This loses user-created metadata (domains, ownership) but preserves the system state discovery.
