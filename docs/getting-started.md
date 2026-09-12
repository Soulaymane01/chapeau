# Getting Started with Chapeau

## Requirements

- **Fedora** (current or recent release)
- **Rust toolchain** (rustc + cargo) — install via [rustup](https://rustup.rs/)
- **DNF5** — the default package manager on Fedora 40+
- **systemd** — comes with Fedora
- **Flatpak** (optional) — for Flatpak application discovery

Chapeau works without Flatpak, but Flatpak application and runtime information will not be discovered if it is not installed.

## Building from source

Clone the repository and build:

```bash
git clone <repository-url> chapeau
cd chapeau
cargo build
```

For an optimized build:

```bash
cargo build --release
```

The binary will be at `target/debug/chapeau` or `target/release/chapeau`.

## Installation

Chapeau does not currently provide a system package or installation script. Run it directly from the build output:

```bash
./target/debug/chapeau --help
```

Or add it to your PATH:

```bash
cp target/release/chapeau ~/.local/bin/
```

## First run

1. **Scan your system:**

```bash
chapeau scan
```

This discovers installed packages (via DNF5), systemd services (via D-Bus), and Flatpak applications (via Flatpak CLI), then stores the results in Chapeau's database.

2. **Check system status:**

```bash
chapeau status
```

This shows a summary of what Chapeau knows about your system.

3. **Open My System:**

```bash
chapeau
```

This shows your intentional resources — the software you actually care about
— grouped by domain when configured, otherwise by type. Use
`chapeau show <resource>` for detail about one resource. See
[my-system.md](my-system.md).

4. **Explore what is intentionally installed:**

```bash
chapeau roots
chapeau packages
```

`chapeau roots` shows the software Chapeau classifies as intentionally
present (see [roots.md](roots.md)). `chapeau packages` lists intentional
packages; `chapeau packages --all` shows the full installed package set with
installation reasons and repository distribution.

## Database location

Chapeau creates its database at:

```
~/.local/state/chapeau/chapeau.db
```

The directory is created automatically on first scan. The database uses SQLite with WAL mode for safe concurrent access.

## Next steps

- Read [usage.md](usage.md) for a practical workflow guide
- Read [commands.md](commands.md) for the full command reference
- Read [architecture.md](architecture.md) to understand how Chapeau works internally
