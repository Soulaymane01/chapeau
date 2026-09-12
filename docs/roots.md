# Intentional Resources (Roots)

Chapeau distinguishes three different things that are easy to conflate:

```text
DNF user-installed
    = evidence that a package was explicitly installed by the user

Chapeau detected root
    = semantic classification of a resource as a top-level,
      user-facing entry point

Chapeau user root
    = explicit, user-declared intent
```

Only the last two are roots. The DNF flag feeds the classifier as one piece
of evidence, but it is not treated as semantic truth.

## Why this distinction exists

On a real Fedora install, DNF marks a very large number of packages as
`User`, because transactions, groups and dependency resolution can all mark
packages as user-installed. A raw list includes development headers, Perl
modules, firmware, shared libraries and filesystem-layout packages alongside
genuine applications:

```text
zsh, zoxide, wireshark, tesseract, postgresql-server, mongodb-org, mpv,
maven, docker-ce, code, cmake, chezmoi, brave-browser, bat, btop ...
```

versus

```text
zlib-ng, yyjson, xz-devel, xorg-x11-proto-devel, wayland-devel,
vulkan-headers, vim-filesystem, qt6-qtbase-devel, libstdc++-devel,
kernel-modules, filesystem ...
```

The first list is what a user recognizes as intentionally installed
software. The second is implementation detail that DNF's flag cannot
distinguish on its own.

## How detection works

Automatic root detection is a small, deterministic, explainable policy
(`src/core/root_policy.rs`). It uses package metadata and the dependency
graph — not a hand-maintained blacklist of package names.

### Candidate evidence

A package is a candidate when it has positive evidence:

| Evidence | Meaning |
|----------|---------|
| DNF user-installed | The package was reported as `reason=User` |
| Desktop application entry | The package ships a `.desktop` file |
| Application bundle | The package ships a payload under `/opt` |
| User-facing executable | The package ships a binary in `/usr/bin` or `/usr/sbin` |
| Canonical project package | The package is the project's metapackage, or the package named after its source RPM whose subpackages provide the entry points |

Installed **Flatpak applications** are always detected roots. Flatpak
runtimes are not.

### Supporting signals

Positive evidence can be outweighed by semantics:

- **Package role.** Derived from RPM metadata (summary text, `-devel` /
  `-headers` / `perl-` / `-filesystem` packaging conventions): development
  packages, libraries and runtimes, firmware, kernel modules, Perl modules,
  build macros, documentation and presentation assets are supporting.
- **Project structure.** Subpackages of one source project are collapsed
  into one representative. `git-core` is supporting because `git` is the
  project package; `docker-ce-cli` is supporting because `docker-ce` is;
  `wireshark-cli` is supporting because `wireshark` is.
- **Toolchain components.** A package that exists only to serve a
  development package of the same project (for example `qt6-linguist`
  required by `qt6-qttools-devel`) is supporting.
- **No entry point.** A user-installed package with no executable,
  no application entry and no canonical project role is supporting.

These rules are additive, and none of them relies on a package name alone.
`libreoffice`, `libreoffice-draw` and `libreoffice-math` remain
applications even though their names superficially resemble library or
development packages.

### Explicit roots always win

`chapeau root add <resource>` marks a resource with source `user`. An
explicit root is kept across scans and is never downgraded or reclassified,
even if automatic detection would not have selected it. If the resource was
previously `detected`, it is promoted to `user`.

`chapeau root remove <resource>` removes only the semantic root state. It
never uninstalls or deletes the resource. A detected root that is removed
can be re-detected by a later scan because detection is recomputed from
current evidence.

## What happens during a scan

`chapeau scan` reconciles roots as follows:

- **Explicit roots** (`user`, `adopted`) are preserved untouched.
- **Detected roots** are fully recomputed from the current system state.
  Detected roots that no longer qualify are removed.
- **Resources are never deleted** because root status disappears. The
  distinction is:

  ```text
  root state  ≠  resource existence
  ```

- **Dependency relationships are never touched** by root reconciliation.
  A supporting package that is a dependency of a root stays protected by
  the dependency graph.
- Every change is recorded in `history`.

Repeated scans are stable: running `chapeau scan` twice does not create
additional roots.

## Commands

```bash
# Semantic list of intentional resources
chapeau roots

# List roots by resource type
chapeau roots list

# Explicitly declare a resource as intentional
chapeau root add <resource> [--reason "..."]
chapeau roots add <resource> [--reason "..."]

# Remove root state (never uninstalls)
chapeau root remove <resource>

# Packages: roots by default, full installed system with --all
chapeau packages
chapeau packages --all

# Same progressive disclosure for services and Flatpaks
chapeau services
chapeau services --all
chapeau flatpaks
chapeau flatpaks --all

# Explain a classification
chapeau why <resource>
```

`chapeau packages --all` continues to expose the complete installed system.
Detection only changes what the default, concise views show — it does not
reduce the underlying database or discovery scope.

## Explainability

`chapeau why <resource>` explains the classification. For a root:

```text
zsh
  Powerful interactive shell

Intentional resource
  Source: detected
  Detected from: DNF user-installed package; user-facing executable
```

For a supporting package:

```text
libstdc++-devel
  Header files and libraries for C++ development

Supporting resource

Not shown as an intentional root because:
  Package role: development package
```

For a user-facing tool that is only a component of a development toolchain
of the same project:

```text
qt6-linguist
  Qt6 Linguist Tools

Observed system resource

Not shown as an intentional root because:
  Package role: application
  Required by (1):
    qt6-qttools-devel
```

The role and install reason are recorded with the package observation during
`chapeau scan`.
