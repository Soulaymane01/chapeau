# Chapeau RPM spec (starting point).
#
# This spec has been syntax-checked with `rpmspec -P` but has NOT been built
# in a mock/koji environment yet. Fedora packaging of Rust projects normally
# uses the cargo macros with vendored crates:
#   https://docs.fedoraproject.org/en-US/packaging-guidelines/Rust/
# Revisit `%build` accordingly before submitting.

Name:           chapeau
Version:        0.1.0
Release:        1%{?dist}
Summary:        Fedora system state and relationship manager

License:        MIT
Source0:        %{name}-%{version}.tar.gz

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

%build
cargo build --release --offline
cargo build --release -p chapeau-gui --offline

%install
install -Dpm0755 target/release/chapeau %{buildroot}%{_bindir}/chapeau
install -Dpm0755 target/release/chapeau-gui %{buildroot}%{_bindir}/chapeau-gui

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
%{_bindir}/chapeau
%{_bindir}/chapeau-gui
%{_datadir}/applications/io.github.Soulaymane01.Chapeau.desktop
%{_datadir}/metainfo/io.github.Soulaymane01.Chapeau.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.Soulaymane01.Chapeau.svg

%changelog
* Sat Sep 12 2026 Chapeau <noreply@example.invalid> - 0.1.0-1
- Initial package
