# Chapeau

A Fedora-native system state and relationship manager that helps you understand what is installed, why it exists, how it is connected, and what happens when you change it.

## The problem

Fedora already knows:

- What packages are installed (DNF/RPM)
- Package dependencies (DNF/RPM)
- Systemd services and their states
- Flatpak applications and runtimes
- Repository configuration

But this information is distributed across several native mechanisms and does not directly answer semantic questions such as:

- **Why is this installed?**
- **Which part of my system uses it?**
- **Is this shared across multiple things?**
- **What would happen if I removed it?**

Chapeau creates a relationship and semantic layer over those facts.

## What Chapeau is NOT

- Not a replacement for DNF
- Not a dependency solver
- Not a replacement for systemd
- Not a Nix/NixOS clone
- Not a package manager
- Not an Ansible replacement

Chapeau is a semantic and organizational layer above Fedora's native system mechanisms.

## Architecture

```
Fedora System
  │
  ├── DNF5 ──────────── Package backend
  ├── systemd ────────── Service backend
  └── Flatpak ────────── Flatpak backend
        │
        ▼
    Discovery
        │
        ▼
    SystemSnapshot
        │
        ▼
    Reconciliation
        │
        ▼
    SQLite Database
        │
        ▼
    Semantic Model
        │
        ├── SystemGraph (petgraph)
        ├── Analysis (orphaned/unused)
        ├── Planning (removal)
        └── Explanation (why)
```

**Core principle:**

> Fedora tells us what exists. Chapeau tells us what it means.

## Quick start

### Requirements

- Fedora (current or recent release)
- Rust toolchain (rustc + cargo)
- DNF5
- systemd
- Flatpak (optional, for Flatpak discovery)

### Build

```bash
git clone <repository-url> chapeau
cd chapeau
cargo build
```

### First run

```bash
# Discover everything on your system
chapeau scan

# See what Chapeau knows
chapeau status

# Explore installed packages
chapeau packages
```

## Commands

| Command | Purpose |
|---------|---------|
| `chapeau scan` | Discover and reconcile Fedora state |
| `chapeau status` | Show Chapeau system state summary |
| `chapeau packages` | List installed packages |
| `chapeau services` | List systemd services |
| `chapeau flatpaks` | List Flatpak applications and runtimes |
| `chapeau repositories` | List DNF repositories |
| `chapeau why <resource>` | Explain why a resource exists |
| `chapeau dependencies <resource>` | Show what a resource depends on |
| `chapeau dependents <resource>` | Show what depends on a resource |
| `chapeau domains list` | List all domains |
| `chapeau domains create <name>` | Create a new domain |
| `chapeau domains add <domain> <resource>` | Add a resource to a domain |
| `chapeau domains remove <domain> <resource>` | Remove a resource from a domain |
| `chapeau orphaned` | Find potentially orphaned resources |
| `chapeau unused` | Find potentially unused resources |
| `chapeau remove <resource>` | Plan and execute safe removal |
| `chapeau backends` | Show backend availability |
| `chapeau db status` | Show database status |

## Typical workflow

```bash
# 1. Scan the machine
chapeau scan

# 2. Check status
chapeau status

# 3. Explore packages
chapeau packages

# 4. Ask why something exists
chapeau why bash

# 5. Organize into domains
chapeau domains create development -d "Development tools"
chapeau domains add development gcc

# 6. Look for orphaned/unused resources
chapeau orphaned
chapeau unused

# 7. Preview a removal
chapeau remove <package>

# 8. After system changes, rescan
chapeau scan
```

## Documentation

- [Getting Started](docs/getting-started.md)
- [Usage Guide](docs/usage.md)
- [Command Reference](docs/commands.md)
- [Architecture](docs/architecture.md)
- [Data Model](docs/data-model.md)
- [Database](docs/database.md)
- [Reconciliation](docs/reconciliation.md)
- [Removal](docs/removal.md)
- [Development](docs/development.md)
- [Testing](docs/testing.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Limitations](docs/limitations.md)

## Development

```bash
cargo check          # Type checking
cargo build          # Build
cargo test           # Run tests
cargo fmt            # Format code
cargo clippy --all-targets --all-features -- -D warnings  # Lint
```

See [Development Guide](docs/development.md) for details.

## License

MIT
