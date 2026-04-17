# Design: Full PagerDuty API Coverage

**Date:** 2026-04-16
**Status:** Implemented

## Problem

`pd` v0.2.1 covers a narrow slice of the PagerDuty REST API: incident types, incident
workflows, workflow triggers, workflow actions, and priorities. Everything else - teams,
users, schedules, escalation policies, services, integrations, event orchestration,
incidents, on-calls, maintenance windows, and more - is only reachable through the raw
`pd rest` passthrough, which requires knowing the exact endpoint, HTTP method, and JSON
body structure from memory.

The goal is a first-class CLI for the full PagerDuty surface, with consistent ergonomics
across every resource.

## Design Principles

### 1. Resources are nouns, operations are verbs

Every command follows the pattern `pd <resource> <operation> [identifier] [flags]`.
Operations are always one of: `list`, `get`, `create`, `update`, `delete`, plus any
resource-specific verbs.

### 2. Positional arg for the primary identifier

The identifier for single-resource operations is always positional, never a flag:

```bash
pd team get Platform           # not: pd team get --name Platform
pd service delete "My Service" # not: pd service delete --id P12345
```

### 3. Name resolution everywhere

Every `get`, `update`, and `delete` resolves the identifier as: PD ID first, then slug,
then display name. Users should never need to look up an opaque ID.

### 4. Positional pattern args for filtering, with tiered fallback matching

Filtering is done via positional patterns, not flags. Zero patterns means "all".
One or more patterns apply in a 3-tier fallback, first tier with any match wins:

1. **Exact match** - any pattern equals the item's name
2. **Starts-with match** - any pattern is a prefix of the item's name
3. **Contains match** - any pattern is a substring of the item's name

Across multiple patterns at the same tier, the semantics are OR (union). This mirrors
the `gx` / `aka` filtering convention.

```bash
pd service list                      # all services
pd service list Platform             # exact match "Platform", or falls back
pd service list data ds              # any service matching "data" or "ds"
pd incident list open                # matches "open" status naturally
pd schedule list "Platform Primary"  # exact match, quoted for spaces
```

For list commands that filter across dimensions (e.g. incidents by team AND status),
flags are added for the *non-name* dimensions:

```bash
pd incident list --status open       # filter by status
pd incident list --team Platform     # filter by team (still matches by positional)
pd incident list --priority P1 P2    # flag takes list values when meaningful
pd oncall list Platform              # filter schedules by positional pattern
```

**Rule of thumb:** positional args filter the resource's primary name. Flags filter on
*other dimensions* (status, priority, team membership) that can't be inferred from a
name.

### 5. Consistent flag names across all resources

Global flags:

| Concept | Flag |
|---------|------|
| Output format | `--output auto\|json\|table` |
| Log level | `-l` / `--log-level` |

Cross-resource filter flags (foreign-key lookups):

| Concept | Flag |
|---------|------|
| Team reference | `--team <pattern>` |
| Service reference | `--service <pattern>` |
| User reference | `--user <email\|id>` |
| Schedule reference | `--schedule <pattern>` |
| Escalation policy reference | `--escalation <pattern>` |

Dimension filter flags:

| Concept | Flag |
|---------|------|
| Status filter | `--status` |
| Priority filter | `--priority` |
| Time range | `--since` / `--until` |

Input/schema flags:

| Concept | Flag |
|---------|------|
| YAML input for complex creates | `--from-file <path>` |
| Print example YAML skeleton | `--example` |

Foreign-key filter flags accept the same 3-tier fallback matching as positional
patterns. So `pd service list --team Platform` uses tiered match to find the team,
then filters services belonging to that team.

### 6. Nested resources use parent-child subcommands

Resources that only exist in the context of a parent use subcommands:

```bash
pd team member list Platform
pd service integration list "My Service"
pd schedule override create "Platform Primary" --user scott --start ... --end ...
pd incident note list <incident-id>
pd incident type field list "Managed Incident"
```

### 7. Short, readable resource names

| Resource | Command | Rationale |
|----------|---------|-----------|
| Escalation Policy | `pd escalation` | "policy" is implied; shorter than full name |
| Maintenance Window | `pd maintenance` | "window" is implied |
| Log Entry | `pd log` | standard abbreviation |
| Change Event | `pd change` | matches PD's own "change events" terminology |
| On-Call | `pd oncall` | single word, natural |
| Alert Grouping Setting | `pd alert-grouping` | hyphenated compound |

---

## Full Command Hierarchy

Where you see `[PATTERNS...]` the command accepts zero or more positional patterns
that filter by name with the 3-tier fallback (exact → starts-with → contains).

```
pd
  # --- Infrastructure ---

  team
    list   [PATTERNS...]
    get    <name-or-id>
    create --name <name> [--description] [--from-file <yaml>] [--example]
    update <name-or-id> [--name] [--description] [--from-file <yaml>]
    delete <name-or-id>
    member
      list   <team> [PATTERNS...]
      add    <team> <user> [--role observer|responder|manager]
      remove <team> <user>

  user
    list [PATTERNS...]
    get  <email-or-id>

  schedule
    list   [PATTERNS...]
    get    <name-or-id>
    create --name <name> --timezone <tz> [--from-file <yaml>] [--example]
    update <name-or-id> [--from-file <yaml>]
    delete <name-or-id>
    override
      list   <schedule> [--since] [--until]
      create <schedule> --user <user> --start <datetime> --end <datetime>
      delete <schedule> <override-id>

  escalation
    list   [PATTERNS...]
    get    <name-or-id>
    create --name <name> --team <team> [--from-file <yaml>] [--example]
    update <name-or-id> [--from-file <yaml>]
    delete <name-or-id>

  service
    list   [PATTERNS...]
    get    <name-or-id>
    create --name <name> --escalation <ep> [--from-file <yaml>] [--example]
    update <name-or-id> [--from-file <yaml>]
    delete <name-or-id>
    integration
      list   <service> [PATTERNS...]
      get    <service> <integration-id>
      create <service> --type <type> [--name <name>] [--from-file <yaml>] [--example]
      delete <service> <integration-id>

  # --- Operational ---

  oncall
    list [PATTERNS...]  # patterns filter by schedule / ep / user name

  incident
    list   [PATTERNS...]  # patterns filter by title
           [--status triggered|acknowledged|resolved]
                          # default: triggered,acknowledged when no --status/--since provided
           [--priority P1 P2 ...]
           [--since] [--until]
    get    <id>
    create --title <title> --service <service>
           [--priority P1..P4] [--type <incident-type>] [--body <body>]
           [--from-file <yaml>] [--example]
    update <id> [--status] [--priority] [--title]
    note
      list <incident-id>
      add  <incident-id> <text>
    alert
      list <incident-id>
      get  <incident-id> <alert-id>
    type
      list   [PATTERNS...] [--filter enabled|disabled|all]
      get    <name-or-id>
      create --name <slug> --display-name <name> [--description]
      update <name-or-id> [--display-name] [--description] [--enabled]
      field
        list   <type> [PATTERNS...]
        create <type> --name <name> --data-type <type> --field-type <type>
    workflow
      list    [PATTERNS...]
      get     <id> [--include-steps]
      create  --name <name> [--description] [--from-file <yaml>] [--example]
      update  <id> [--name] [--description]
      delete  <id>
      enable  <id>
      disable <id>
      export  <id> [--real-id <id>]
      import  <file> [--id <id>]
    trigger
      list   [PATTERNS...]  # patterns filter by workflow name
      get    <id>
      create --workflow <id> --type conditional|manual|incident-type
             [--condition <pcl>] [--incident-types <types>]
             [--from-file <yaml>] [--example]
      update <id> [--condition] [--incident-types]
      delete <id>
      bind   <trigger-id> --service <service>
      unbind <trigger-id> --service <service>

  # --- Automation / Config ---

  priority
    list
    verify

  orchestration
    list [PATTERNS...]
    get  <name-or-id>
    router
      get    <orchestration>
      update <orchestration> --from-file <path>   # YAML or JSON definition of router rules

  maintenance
    list   [PATTERNS...]
    get    <id>
    create --service <service> --start <datetime> --end <datetime>
           [--description] [--from-file <yaml>] [--example]
    update <id> [--start] [--end] [--description]
    delete <id>

  alert-grouping
    list   [PATTERNS...]
    get    <id>
    create --service <service> --type <type> [--from-file <yaml>] [--example]
    update <id> [--from-file <yaml>]
    delete <id>

  # --- Observability ---

  log
    list [PATTERNS...] [--since] [--until]
    get  <id>

  change
    list   [PATTERNS...] [--since] [--until]
    get    <id>
    create --summary <text> --service <service> [--links <json>]

  # --- Passthrough ---

  action
    list [PATTERNS...]
    get  <id>

  cache
    clear [<resource-type>]   # flush name-resolution ID cache

  rest <METHOD> <PATH> [--body <json>]
```

---

## Key Ergonomic Decisions

### `pd escalation` not `pd ep` or `pd escalation-policy`

`ep` is too opaque for newcomers. `escalation-policy` is accurate but verbose.
`escalation` is readable and unambiguous in context - there's nothing else in PD called
an escalation.

### Triggers move under `pd incident trigger`

Triggers are incident workflow triggers, not a general-purpose concept. Moving them under
`pd incident trigger` makes the ownership clear. `bind`/`unbind` replace the verbose
`create-for-service`/`remove-from-service`.

### `pd oncall list` with filters

On-call is a derived view, not a CRUD resource. `list` is the only operation; filters
narrow to a schedule, team, EP, or user. This answers "who is on call right now?" in one
command.

### `--from-file <yaml>` for complex create/update, `--example` to scaffold

Escalation policies, services, and orchestration routers have deeply nested config that
doesn't map cleanly to flags. These accept `--from-file yaml` for the full payload,
with flags for the top-level required fields. This mirrors the workflow `import` pattern
already in use.

`--from-file -` reads from stdin, so the full shell pipeline is supported:

```bash
some-tool | pd escalation create --from-file -
```

CLI flags take absolute precedence over file contents. If `--name NewName` is passed
alongside a file that contains `name: OldName`, the flag wins. For collection fields,
flags overwrite - they do not merge with file contents.

Every command that accepts `--from-file` also accepts `--example`, which prints a
fully-commented YAML skeleton to stdout and exits. The skeleton includes:

- All required fields with placeholder values
- All optional fields, commented out, with descriptions
- Realistic example values pulled from the PD schema

```bash
pd escalation create --example > platform-ep.yml
# edit platform-ep.yml
pd escalation create --from-file platform-ep.yml
```

This lets users learn the schema without reading API docs, and gives them a
version-controllable starting point.

### Output auto-detection via isatty

The `--output auto` mode (the default) inspects stdout at runtime:

- **TTY detected** - render human-readable tables, colors enabled
- **Pipe / redirect detected** - emit newline-delimited JSON, no color

This means `pd service list | jq '.[].name'` works without any flags. Scripts get
structured data by default; interactive sessions get readable output.

`--output json` and `--output table` override the detection explicitly.

### Name resolution ID cache

Name-to-ID lookups (slug and display name searches) are cached at:

```
~/.cache/pd/ids/<resource-type>.json
```

Cache TTL: 5 minutes. Cache is invalidated on any write operation to that resource
type (create, update, delete). The cache is transparent - it never changes the
result, only the speed of resolution.

Cache writes are atomic (write to temp file, rename over target) to prevent
corruption from concurrent `pd` invocations. If a cached ID returns a 404 (resource
deleted out-of-band via the PagerDuty UI), the cache for that resource type is
transparently invalidated and name resolution retries against a fresh API call before
surfacing an error.

**Cache population strategy:** on a cache miss, name resolution uses the API `query`
parameter where the endpoint supports it (see filtering section below) to fetch only
matching candidates rather than paginating all resources. For endpoints without `query`
support, a full paginated fetch is required; results are cached to amortize the cost.

**Negative caching:** a failed lookup (name not found after full resolution) is cached
as a miss for the remainder of the TTL. This prevents repeated API calls when the same
nonexistent name is referenced in a script loop.

**Nested resources are excluded from caching.** Resources like `service integration`
and `incident type field` have names that are only unique within a parent context (e.g.,
multiple services may have an integration named "Datadog"). They are always fetched in
the context of a known parent, making the result set small; caching adds complexity
without benefit.

`pd cache clear` empties the cache. `pd cache clear <resource-type>` clears a
specific resource.

This ensures `pd service get "My Service"` doesn't incur a list call on every
invocation in a tight loop or script.

### Positional filtering semantics (3-tier fallback)

Copied from `gx`/`aka` with the third tier added. When one or more patterns are passed
to a `list` command:

```rust
fn filter<T>(items: &[T], patterns: &[String], name_of: fn(&T) -> &str) -> Vec<&T> {
    if patterns.is_empty() {
        return items.iter().collect();
    }

    // Tier 1: exact match on name
    let t1: Vec<_> = items.iter().filter(|i| patterns.iter().any(|p| name_of(i) == p)).collect();
    if !t1.is_empty() { return t1; }

    // Tier 2: starts-with
    let t2: Vec<_> = items.iter().filter(|i| patterns.iter().any(|p| name_of(i).starts_with(p))).collect();
    if !t2.is_empty() { return t2; }

    // Tier 3: contains
    items.iter().filter(|i| patterns.iter().any(|p| name_of(i).contains(p))).collect()
}
```

Patterns are OR'd together. First tier with any match wins and returns all items at
that tier. Consistent behavior across every resource that supports `list` filtering.
This lives as a shared helper in `src/filter.rs`.

**API-first filtering:** structured flags (`--status`, `--team`, `--user`, `--priority`,
`--since`, `--until`) are passed directly to PagerDuty REST API query parameters to
reduce the response payload before local pattern filtering applies.

Positional name patterns are forwarded to the API `query` parameter on endpoints that
support it, then the 3-tier local fallback is applied to the (already-narrowed) result
set. Endpoints without `query` support require a full paginated fetch; local filtering
runs client-side only.

| Supports API `query` | Does not support API `query` |
|---|---|
| `escalation`, `service`, `team`, `user` | `incident`, `oncall`, `log`, `change` |
| `schedule`, `maintenance`, `incident workflow` | `orchestration`, `alert-grouping`, `action` |

### `pd incident create` is not skipped

Creating incidents programmatically is a real use case (testing, scripted response,
integration smoke tests). It's included but clearly documented as a mutating operation.

### `action list` has no `--query` flag

The shakedown revealed that `--query` on action list returns a 400 from the PD API.
The flag is removed. Filtering is done client-side via `jq`.

### Status pages are out of scope

Status page management (posts, updates, severities) is a separate product surface and
not commonly managed via CLI. Covered by `pd rest` passthrough.

---

## Implementation Phases

### Phase 1 - Fix existing bugs (v0.2.2)

- Remove `--query` from `action list`
- Fix `incident type field list` to resolve display names like `get` does
- Note in `incident workflow get` when a stub with no steps is returned

### Phase 2 - Infrastructure layer (v0.3.0)

Add native commands for: `team`, `user`, `schedule`, `escalation`, `service` with
`integration` subresource. Add `oncall`.

**Open items carried forward from Phase 2**:

- **`--team` filter on `user list`, `escalation list`, `service list`** landed
  in v0.3.2 via a follow-up pass (tiered team-name resolution plus the
  PagerDuty `team_ids[]` query parameter). Other cross-resource filter flags
  (`--service`, `--user`, `--schedule`, `--escalation`) on list commands
  remain as needed when additional use cases surface.
- **Name-resolution ID cache** (design "Name resolution ID cache" section).
  Transparent `~/.cache/pd/ids/<resource-type>.json` with 5-minute TTL,
  atomic writes, negative caching, and `pd cache clear` remain deferred
  until measurable perf pain. Every `resolve_*` helper currently hits the
  API on every call.

### Phase 3 - Operational layer (v0.4.0)

Add native commands for: `incident list/get/create/update`, `incident note`,
`incident alert`.

Move `incident trigger` from top-level `trigger` (deprecate old path).

**Implemented in v0.4.0**:

- `pd incident list` / `get` / `create` / `update` with tiered filtering,
  default `statuses[]=triggered,acknowledged` when no `--status`/`--since`,
  and `--team`/`--priority` cross-resource filters
- `pd incident note list` / `add` (accepts `-` for stdin text)
- `pd incident alert list` / `get`
- `pd incident trigger list|get|create|update|delete|bind|unbind` - the
  create flag is `--workflow` (not the old `--workflow-id`); `bind`/`unbind`
  take `--service` with tiered name resolution
- Top-level `pd trigger` kept as deprecated alias with a stderr warning
- `From:` header wiring on the HTTP client via `post_with_from` /
  `put_with_from`; requester email resolves as `--from` > `PAGERDUTY_FROM_EMAIL`
  > `from-email` config key

**Open items carried forward from Phase 3**:

- **Name-resolution ID cache** (design "Name resolution ID cache" section).
  Still deferred - `resolve_priority_id`, `resolve_incident_type_id`, and
  the existing `resolve_*` helpers all hit the API on every call.
- **Legacy `resolve_incident_type_ids` duplication.** The plural helper now
  exists in both `src/resources/trigger.rs` (legacy) and the new
  `src/resources/incident/trigger.rs`. Collapse when the legacy top-level
  `pd trigger` is removed.

### Phase 4 - Automation/Observability layer (v0.5.0)

Add: `maintenance`, `alert-grouping`, `orchestration router`, `log`, `change`.

**Implemented in v0.5.0**:

- `pd maintenance list|get|create|update|delete`. List supports
  `--team`/`--service` cross-resource filters and the `query` API param;
  create supports repeatable `--service`, `--start`/`--end`, `--description`,
  and `--from-file` YAML with `--example` skeleton.
- `pd alert-grouping list|get|create|update|delete` against
  `/alert_grouping_settings`. No `query` support on the endpoint, so
  patterns filter client-side. YAML skeleton exposes the free-form
  `config` block so users can set grouping-type-specific config without
  memorizing the API shape.
- `pd orchestration list|get` and `pd orchestration router get|update`.
  Router update accepts YAML or JSON via `--from-file -` (stdin).
- `pd log list|get` against `/log_entries` with `--since`/`--until`.
- `pd change list|get` against `/change_events` with `--service`,
  `--since`, `--until`.

**Open items carried forward from Phase 4**:

- **`pd change create` was scoped out.** PagerDuty change events are
  created via the Events API v2
  (`POST https://events.pagerduty.com/v2/change/enqueue`) with an
  integration routing key, not the REST API + Token auth. Adding a
  second code path to `PdClient` (different base URL, routing-key auth)
  is deferred until demand materializes. Use `pd rest` passthrough
  against the Events API as a workaround.
- **Name-resolution ID cache** (design "Name resolution ID cache"
  section). Still deferred. `resolve_*` helpers hit the API on every
  invocation.
- **Shakedown risks to validate against the live API** (not covered by
  wiremock tests, since they're PagerDuty contract issues rather than
  code bugs):
  - `/event_orchestrations` list envelope key may be
    `event_orchestrations` rather than `orchestrations`. If so, update
    both call sites in `src/resources/orchestration.rs`.
  - `/event_orchestrations` and `/alert_grouping_settings` may reject
    `?offset=` the way `/incident_workflows/triggers` does. If a 400
    surfaces, switch to `get_all_no_offset`.
  - `pd maintenance update` currently sends a partial PUT (no fetch
    merge). If PD requires the full object, use the fetch-then-mutate
    pattern already in `src/resources/team.rs`.

---

## What stays the same

- Global flags (`--output`, `--api-token`, `-l`)
- Auth resolution order (flag > env > config file)
- Config file location (`~/.config/pagerduty-cli/pagerduty-cli.yml`)
- Log file location (`~/.local/share/pagerduty-cli/logs/pagerduty-cli.log`)
- `pd rest` passthrough (permanent escape hatch)
- Table/JSON/auto output format behavior
