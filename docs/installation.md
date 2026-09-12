# Installing Chapeau

Chapeau ships two binaries built from one source tree:

- `chapeau` — terminal interface
- `chapeau-gui` — graphical interface (GTK4 + libadwaita)

There is no official package yet; installation is from source.

## Requirements

| Requirement | Needed for |
|-------------|------------|
| Fedora (current or recent) | DNF5, systemd, Flatpak discovery |
| Rust toolchain (`rustc`, `cargo`) | building |
| `gtk4-devel`, `libadwaita-devel` | building `chapeau-gui` |
| `graphviz` | the graph view (optional) |

```bash
sudo dnf install rust cargo gtk4-devel libadwaita-devel graphviz
```

## Build and install

```bash
git clone https://github.com/Soulaymane01/chapeau.git
cd chapeau

cargo build --release                 # chapeau
cargo build --release -p chapeau-gui  # chapeau-gui

install -Dm755 target/release/chapeau     ~/.local/bin/chapeau
install -Dm755 target/release/chapeau-gui ~/.local/bin/chapeau-gui
```

`~/.local/bin` must be in your `PATH`. (Use `/usr/local/bin` with `sudo` for
a system-wide install.)

## Desktop entry

```bash
install -Dm644 gui/resources/io.github.Soulaymane01.Chapeau.desktop \
    ~/.local/share/applications/io.github.Soulaymane01.Chapeau.desktop
install -Dm644 gui/resources/io.github.Soulaymane01.Chapeau.metainfo.xml \
    ~/.local/share/metainfo/io.github.Soulaymane01.Chapeau.metainfo.xml
install -Dm644 gui/resources/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg \
    ~/.local/share/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg

update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

Chapeau then appears in your application menu.

## First run

```bash
chapeau scan          # build the model (a few seconds to a minute)
chapeau               # My System overview
chapeau-gui           # graphical interface
```

Everything lives in one database:

```text
~/.local/state/chapeau/chapeau.db
```

Chapeau never removes anything on its own, and every scan is idempotent.

## Updating

```bash
git pull
cargo build --release
cargo build --release -p chapeau-gui
install -Dm755 target/release/chapeau     ~/.local/bin/chapeau
install -Dm755 target/release/chapeau-gui ~/.local/bin/chapeau-gui
```

## Uninstalling

```bash
rm ~/.local/bin/chapeau ~/.local/bin/chapeau-gui
rm ~/.local/share/applications/io.github.Soulaymane01.Chapeau.desktop
rm ~/.local/share/metainfo/io.github.Soulaymane01.Chapeau.metainfo.xml
rm ~/.local/share/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg
rm -r ~/.local/state/chapeau   # the model and history
```

Removing the state directory only deletes Chapeau's knowledge about your
system; it never touches installed software.

## RPM

`packaging/chapeau.spec` builds a working RPM with vendored dependencies:

```bash
sudo dnf install rust cargo gtk4-devel libadwaita-devel desktop-file-utils

mkdir -p rpmbuild/SOURCES
git archive --format=tar.gz --prefix=chapeau-0.1.0/ \
    -o rpmbuild/SOURCES/chapeau-0.1.0.tar.gz HEAD

cargo vendor-filterer --platform x86_64-unknown-linux-gnu /tmp/chapeau-vendor/vendor
tar -cJf rpmbuild/SOURCES/vendor.tar.xz -C /tmp/chapeau-vendor vendor

rpmbuild -ba --define "_topdir $(pwd)/rpmbuild" packaging/chapeau.spec
```

The RPM installs `chapeau`, `chapeau-gui`, the desktop entry, AppStream
metadata and the icon. See [Chapeau GUI](gui.md#packaging-status).
