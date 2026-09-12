# Chapeau Data Model

## Overview

Chapeau's data model represents a Fedora system as a directed graph of resources connected by typed relationships, organized into logical domains, with observed state and provenance metadata.

## Core concepts

### Resource

A resource is anything on the system that Chapeau tracks. Resources are identified by their type and native identifier.

**Resource types:**

| Type | Description | Native system | Native ID example |
|------|-------------|---------------|-------------------|
| `Package` | RPM package | DNF5 | `bash`, `python3` |
| `Service` | systemd service unit | systemd D-Bus | `sshd.service` |
| `Flatpak` | Flatpak application or runtime | Flatpak CLI | `org.mozilla.firefox` |
| `Repository` | DNF repository | DNF5 | `fedora`, `updates` |

**Resource fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Chapeau's unique identifier |
| `resource_type` | ResourceType | One of the four types above |
| `native_id` | String | Identifier in the source system |
| `display_name` | String (optional) | Human-readable name |
| `created_at` | ISO-8601 | When the resource was first recorded |
| `updated_at` | ISO-8601 | When the resource was last updated |

Resources are unique by `(type, native_id)`. The same `native_id` can exist across different types (e.g., a package named `redis` and a service named `redis.service`).

### Relationship

A relationship is a directed edge between two resources (or between a domain and a resource).

**Relationship types:**

| Type | Meaning | Example |
|------|---------|---------|
| `depends_on` | Source requires target to function | `vim` depends_on `glibc` |
| `provides` | Source fulfills or supplies target's interface | `python3` provides `python3.12`; a package provides the services it ships (`postgresql-server` provides `postgresql.service`) |
| `owns` | Source has authoritative ownership of target | Domain `AI` owns `pytorch` |
| `uses` | Source interacts with or utilizes target | Domain `dev` uses `gcc` |
| `comes_from` | Source was obtained from or produced by target | `vim` comes_from `fedora` |

**Relationship origins:**

| Origin | Meaning |
|--------|---------|
| `system` | Discovered from system state (e.g., RPM dependencies, systemd Wants) |
| `user` | Created explicitly by the user (e.g., domain assignments) |
| `derived` | Computed or inferred by Chapeau |

**Relationship fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Unique identifier |
| `source_id` | String | ID of the source resource |
| `relationship_type` | RelationshipType | The type of relationship |
| `target_id` | String | ID of the target resource |
| `origin` | RelationshipOrigin | How the relationship was created |
| `created_at` | ISO-8601 | Creation timestamp |
| `updated_at` | ISO-8601 | Last update timestamp |

Relationships are unique by `(source_id, relationship_type, target_id)`.

**Important:** Reverse dependency relationships (e.g., "what depends on X") are not stored separately. They are computed by querying incoming `depends_on` edges. This avoids data duplication and inconsistency.

### Domain

A domain is a logical organizational unit. It is metadata — not a system resource.

Domains group resources to help answer questions like "what is this for?" or "who owns this?"

**Domain fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Unique identifier |
| `name` | String | Domain name (unique) |
| `description` | String (optional) | Human-readable description |
| `created_at` | ISO-8601 | Creation timestamp |
| `updated_at` | ISO-8601 | Last update timestamp |

**Domain-resource relationships:**

Domains connect to resources via the `domain_resources` table:

| Field | Description |
|-------|-------------|
| `domain_id` | Domain UUID |
| `resource_id` | Resource UUID |
| `relationship` | `owns` or `uses` |
| `reason` | Optional human-readable reason |

**Ownership vs. usage:**

- `owns` — The domain has authoritative ownership. Removing the domain concept does not mean removing the resource.
- `uses` — The domain interacts with or utilizes the resource.

Example:
```
AI domain owns pytorch
Development domain uses python3
```

Removing the AI domain's ownership of pytorch does not mean pytorch is unused — Development still uses it.

### Observation

An observation is a cached snapshot of a resource's current state in the live system.

**Observation fields:**

| Field | Type | Description |
|-------|------|-------------|
| `resource_id` | String | Resource UUID (primary key) |
| `observed_at` | ISO-8601 | When this observation was recorded |
| `installed` | bool (optional) | Whether the resource is installed |
| `version` | String (optional) | Current version |
| `active` | bool (optional) | Whether a service is active |
| `enabled` | bool (optional) | Whether a service is enabled |
| `failed` | bool (optional) | Whether a service has failed |
| `metadata` | JSON (optional) | Additional state data (e.g., Flatpak branch, arch) |

Observations are updated on each scan. They represent what Fedora reports, not what Chapeau declares.

When a complete discovery run no longer reports a resource, its observation is
updated to `installed = false` (the resource and its semantic state are kept).
This is how missing explicit roots are detected. Only the owning backend being
successfully queried allows absence to be inferred.

### Provenance

Provenance records how a resource was installed or obtained.

**Provenance fields:**

| Field | Type | Description |
|-------|------|-------------|
| `resource_id` | String | Resource UUID (primary key) |
| `installation_method` | String (optional) | How it was installed (e.g., `dnf`) |
| `installation_time` | String (optional) | When it was installed |
| `transaction_id` | String (optional) | DNF transaction ID |
| `source_type` | String (optional) | Source type (e.g., `repository`) |
| `source_id` | String (optional) | Source identifier (e.g., repo name) |
| `adopted_at` | String (optional) | When Chapeau first recorded it |

Provenance is distinct from ownership. "Installed via DNF" does not mean "owned by a domain."

### Root

A root is a semantic statement that a resource is intentionally present as a
top-level, user-facing entry point — not merely that it exists or was
installed by DNF.

**Root sources:**

| Source | Meaning | Survives scan reconciliation |
|--------|---------|------------------------------|
| `user` | Explicitly declared with `chapeau root add` | Always |
| `adopted` | User adopted an existing resource as a root | Always |
| `detected` | Automatically classified from evidence | Recomputed every scan |

**Root fields:**

| Field | Type | Description |
|-------|------|-------------|
| `resource_id` | String | Resource UUID (primary key) |
| `source` | String | `user`, `adopted` or `detected` |
| `reason` | String (optional) | Evidence recorded when the root was created |
| `created_at` | ISO-8601 | When root state was first recorded |
| `updated_at` | ISO-8601 | When root state was last updated |

The critical distinction:

```text
root state  ≠  resource existence
```

Removing root status never deletes a resource, observation, relationship or
domain association. Conversely, a resource can exist without being a root —
most do. See [Intentional Resources (Roots)](roots.md) for the
classification policy.

## Conceptual distinctions

### Resource vs. Domain

A resource is a concrete system entity (package, service, flatpak, repository). A domain is an organizational concept created by the user.

```
AI                    ← domain (conceptual)
Development           ← domain (conceptual)
python3               ← resource (actual system package)
postgresql.service    ← resource (actual systemd service)
```

### Observed vs. Declared

- **Observed state** comes from Fedora (package versions, service states, Flatpak installations). Stored in observations.
- **Declared state** is user-created metadata in Chapeau (domains, ownership, usage relationships). Stored in domain_resources and relationships with origin `user`.

### Ownership vs. Dependency

```
AI
 └── owns → PyTorch

PyTorch
 └── depends_on → Python

Development
 └── uses → Python
```

Removing AI's ownership of PyTorch does not mean removing Python. Python is also used by the Development domain and depended on by other packages.

### Authoritative sources

| What | Authoritative source |
|------|---------------------|
| What packages are installed | DNF5 / RPM |
| Package dependencies | DNF5 / RPM |
| Service states | systemd / D-Bus |
| Flatpak installations | Flatpak CLI |
| Repository configuration | DNF5 |
| Why something exists | Chapeau (domains, ownership) |
| What depends on what | Chapeau (relationships) |
| User intent (domain assignments, explicit roots) | Chapeau (`domain_resources`, `roots` with source `user`) |
| Intentional-resource classification | Chapeau (package metadata + dependency graph + DNF evidence) |
