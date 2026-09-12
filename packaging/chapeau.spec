# Chapeau RPM spec.
#
# Self-contained: the crate dependencies ship as a vendored tarball
# (Source1), so no extra RPM macros are required. Fedora's rust-packaging
# "cargo_prep"/"cargo_build" macros can replace the manual vendor setup
# when submitting to Fedora.
#
# Build with:
#   rpmbuild -ba --define "_topdir $(pwd)/rpmbuild" packaging/chapeau.spec
# or in an isolated buildroot:
#   mock -r fedora-44-x86_64 chapeau-0.1.0-1.fc44.src.rpm

%global debug_package %{nil}

Name:           chapeau
Version:        0.1.0
Release:        1%{?dist}
Summary:        Fedora system state and relationship manager

License:        MIT
URL:            https://github.com/Soulaymane01/chapeau
Source0:        %{name}-%{version}.tar.gz
Source1:        vendor.tar.xz

BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  desktop-file-utils
Requires:       dnf5
Requires:       systemd
Recommends:     graphviz
Recommends:     flatpak

%description
Chapeau is a semantic layer over Fedora's native system mechanisms. It
observes DNF, systemd and Flatpak, records relationships, and explains what is
installed, why it is there, what depends on it, and what would change if it
were removed. This package ships the terminal frontend (chapeau) and the
graphical frontend (chapeau-gui).

%prep
%autosetup -n %{name}-%{version}
tar -xJf %{SOURCE1}
mkdir -p .cargo
cat > .cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF

%build
cargo build --release --offline
cargo build --release --offline -p chapeau-gui

%install
install -Dpm0755 target/release/chapeau \
    %{buildroot}%{_bindir}/chapeau
install -Dpm0755 target/release/chapeau-gui \
    %{buildroot}%{_bindir}/chapeau-gui

install -Dpm0644 gui/resources/io.github.Soulaymane01.Chapeau.desktop \
    %{buildroot}%{_datadir}/applications/io.github.Soulaymane01.Chapeau.desktop
install -Dpm0644 gui/resources/io.github.Soulaymane01.Chapeau.metainfo.xml \
    %{buildroot}%{_datadir}/metainfo/io.github.Soulaymane01.Chapeau.metainfo.xml
install -Dpm0644 gui/resources/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg

%check
desktop-file-validate \
    %{buildroot}%{_datadir}/applications/io.github.Soulaymane01.Chapeau.desktop

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/chapeau
%{_bindir}/chapeau-gui
%{_datadir}/applications/io.github.Soulaymane01.Chapeau.desktop
%{_datadir}/metainfo/io.github.Soulaymane01.Chapeau.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg

%changelog
* Sat Sep 12 2026 Soulaymane01 <noreply@github.com> - 0.1.0-1
- Initial package
