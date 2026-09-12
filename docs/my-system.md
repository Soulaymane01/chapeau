# My System

"My System" is Chapeau's default view: the software you intentionally have,
not the thousands of supporting packages underneath it.

The philosophy is progressive disclosure:

```text
chapeau                  intentional resources (this document)
  ↓
chapeau show <resource>  details about one resource
  ↓
chapeau dependencies     the graph underneath it
chapeau why
  ↓
chapeau packages --all   the full tracked system
```

Everything is always tracked in the database. The overview only changes what
is shown by default.

## The overview

```bash
chapeau
chapeau overview
```

Running `chapeau` with no command prints the overview.

### Before domains are configured

Roots are grouped by resource type:

```text
My System
=========

Packages (63)
  bat                              [detected]
  docker-ce                        [detected]
  zsh                              [detected]
  ...

Flatpak (8)
  Firefox (org.mozilla.firefox)    [detected]
  ...

71 intentional resources · 3835 tracked resources · 20644 relationships

Inspect:   chapeau show <resource> · chapeau why <resource> · chapeau drift
Explore:   chapeau packages --all · chapeau services --all · chapeau flatpaks --all
Organize:  chapeau domains create <name> · chapeau domains add <domain> <resource>
```

### After domains are configured

Domains are user-declared organization, so they take precedence when present:

```text
My System
=========

databases (2)
  mongodb-org       [detected]
  postgresql-server [detected]

development (2)
  code [detected]
  zsh  [detected]

Not in a domain (67)
  bat
  docker-ce
  ...

71 intentional resources · 3835 tracked resources · 20644 relationships
```

A root can belong to several domains and is shown in each. The ungrouped list
is truncated in the overview; `chapeau roots` lists everything.

Missing intentional resources are tagged `[user, missing]` and counted in the
summary. See [Intentional Resources (Roots)](roots.md#missing-roots).

## Resource detail

```bash
chapeau show <resource>
```

`show` answers "what is this, and where does it sit in my system?" — facts,
intent, organization and immediate relationships.

### A package

```text
postgresql-server
  The programs needed to create and run a PostgreSQL server

Type:         package
Version:      18.3-2.fc44
Recorded:     installed
Origin:       updates
Intent:       intentional (detected)
              DNF user-installed package

Dependencies (15):
  glibc
  openssl-libs
  ...

Inspect further:
  chapeau why postgresql-server
  chapeau dependencies postgresql-server
  chapeau remove postgresql-server   (preview removal impact)
```

### A supporting package

```text
glibc
  The GNU libc libraries

Type:         package
Version:      2.43-6.fc44
Recorded:     installed
Origin:       updates
Intent:       not a root (role: library or runtime)

Dependencies (3):
  filesystem
  glibc-common
  libgcc

Required by (2065):
  ...
```

### A service

```text
firewalld - dynamic firewall daemon
  firewalld.service

Type:         service
State:        active, enabled
Intent:       not a root
```

### A Flatpak application

```text
Obsidian
  md.obsidian.Obsidian

Type:         flatpak
Version:      1.12.7
Branch:       stable
Recorded:     installed
Origin:       flathub
Intent:       intentional (detected)
              Flatpak application

Uses (1):
  Freedesktop Platform
```

### A repository

```text
fedora

Type:         repository
Packages:     1903
```

### A missing intentional resource

```text
postgresql-server
  PostgreSQL server and client

Type:         package
Version:      16.4-1.fc44 (last seen)
Recorded:     missing — not currently present on the system
Intent:       intentional (user)

Intentional but missing:
  This resource is recorded as intentional, but it is not currently
  present on the system.
  Reinstall it, or run 'chapeau root remove postgresql-server' if it is no
  longer wanted.
```

## How `show` relates to the other commands

| Command | Answers |
|---------|---------|
| `chapeau show <resource>` | What is this resource and where does it sit? |
| `chapeau why <resource>` | Why is it here, and why is (or isn't) it a root? |
| `chapeau dependencies <resource>` | What does it depend on? |
| `chapeau dependents <resource>` | What depends on it? |
| `chapeau drift` | What changed since the last scan? |

## Domains

Domains are intentional user metadata, not automatic classification.
Chapeau never invents or populates domains for you:

```bash
chapeau domains create databases
chapeau domains add databases postgresql-server --owns --reason "primary database"
chapeau domains add development zsh
```

The overview groups roots by domain only after you create one. See
[Command Reference](commands.md#domains).
