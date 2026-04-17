# Design Document: Shakedown of v0.5.0

**Author:** Scott Idler
**Date:** 2026-04-16
**Status:** Implemented
**Review Passes Completed:** 5/5 + 2 Architect consults

## Summary

Close every gap the Gemini Architect audit and Claude's codebase review
surfaced after the full-API-coverage design shipped at v0.5.0. Fifteen
concrete items spanning correctness bugs, spec deviations, deferred
features, code cleanup, documentation drift, and test gaps. One design
doc, phased rollout, no half-measures.

## Problem Statement

### Background

The full-API-coverage design doc
(`docs/design/2026-04-16-full-api-coverage.md`) shipped in four phases as
`pd` v0.2.2 → v0.5.0. An Implementation Audit by the Gemini Architect
persona, cross-referenced with Claude's own codebase knowledge, identified
fifteen items where the implementation falls short of the stated design,
silently degrades performance, defers advertised features, or leaves
tests and docs drifting from the code.

### Problem

v0.5.0 marks the original design doc as "Implemented," but:

- Three correctness bugs exist that will surface the first time a user
  exercises the affected commands against the live PagerDuty API.
- Three design claims are not faithfully delivered (multi-pattern
  `?query` forwarding, case-insensitive filtering, name-resolution ID
  cache).
- One command (`pd change create`) was scoped out without a replacement.
- One deprecated code path (`pd trigger` + `src/resources/trigger.rs`)
  lingers as a stderr warning nobody wants.
- The README and parts of the original design doc contradict the code.
- Three test gaps leave user-facing behavior under-covered.

Shipping a v0.5.0 that is "implemented per design" but fails this audit
is misleading. Fix every item before calling the API coverage design
done.

**The fifteen items, for reference:**

1. Orchestration envelope key (`orchestrations` vs `event_orchestrations`)
2. Cursor pagination on `/event_orchestrations` and
   `/alert_grouping_settings` (replacing the 100-item `get_all_no_offset`)
3. `pd maintenance update` partial PUT
4. Multi-pattern `?query` forwarding (semaphore-capped)
5. Case-insensitive filtering
6. Name-resolution ID cache (per-entry files, subdomain-namespaced,
   granular invalidation)
7. `pd change create` via Events API v2 (with dynamic routing-key
   discovery from `--service`)
8. `pd cache clear` command
9. Delete legacy `src/resources/trigger.rs`
10. Remove top-level `pd trigger`
11. README `--query` flag drift on `action list` and
    `incident workflow list`
12. Original design doc's `action list` description vs. actual behavior
13. Wiremock test for `incident create` `From:` precedence
14. Wiremock test for `incident list --until` alone preserving default
    statuses
15. Smoke test asserting `post` without `From:` does not emit the header

### Goals

- Every item in the audit is implemented or explicitly documented as
  "will never be done and here is the permanent workaround."
- The original full-API-coverage design doc's "Open Items" sections are
  emptied (or rewritten to reflect reality) on completion.
- No new design debt is introduced: every fix is landed with tests.
- Breaking changes are justified and version-bumped accordingly
  (v0.5.0 → v0.6.0 for removals; v0.5.0 → v0.5.1 for bugfixes).

### Non-Goals

- New features beyond what the original design doc promised. This is a
  close-the-gaps doc, not a v2 feature expansion.
- Status page management (still out of scope per original design).
- Reworking the client's single-base-URL auth beyond what `pd change
  create` via Events API v2 requires.
- Telemetry, metrics, or observability beyond the existing tracing.

## Proposed Solution

### Overview

Three buckets of work, phased:

1. **v0.5.1 (bugfixes + doc drift + test gaps):** correctness bugs,
   README cleanup, missed tests. No CLI surface changes.
2. **v0.6.0 (breaking cleanup + deferred design work):** remove legacy
   `pd trigger`, land case-insensitive filter, implement multi-pattern
   `?query` forwarding, implement the name-resolution ID cache with
   `pd cache clear`.
3. **v0.7.0 (new client surface):** extend `PdClient` for Events API v2
   + routing-key auth, wire `pd change create`.

### Architecture

Most items are in-place changes to existing resource modules. Three
items require structural additions:

- **`src/cache.rs`**: new module for the name-resolution ID cache
  *backend*. Per-entry file layout under XDG cache dir:
  `~/.cache/pd/ids/<subdomain>/<resource-type>/<name-hash>.json`.
  Account-namespaced by subdomain so staging and production tokens
  never share IDs. One file per (resource-type, name) eliminates the
  concurrent-writer lost-update race: two processes caching *different*
  names can never collide, and two processes caching the *same* name
  produce the same content (ID is deterministic from name at one point
  in time).
  - 5-minute TTL per entry, evaluated at read.
  - Atomic write per entry (write to `<file>.tmp`, `sync_all`, rename).
  - **No negative caching.** Failed lookups are not persisted. The
    original "tight loop against a misspelled name" motivation is
    outweighed by the CI-pipeline failure mode where an external
    provisioner (Terraform, another `pd` session, the PD UI) creates
    the resource while a stale negative entry lives. PD's own
    retry-with-backoff absorbs any real-world misspelling loop cost.
  - `<name-hash>` is the full `sha256(name)` as 64 hex chars. Full
    hash (not a truncated prefix) eliminates the hash-collision
    failure mode entirely; the overhead of 48 extra filename bytes
    per entry is negligible on any modern filesystem.
  - Exposed as:
    - `fn get(type, name) -> Option<String>` (the ID, or None)
    - `fn put(type, name, id)`
    - `fn invalidate_entry(type, name)` - delete one entry file
    - `fn invalidate_type(type)` - rmrf the resource-type directory
    - `fn invalidate_subdomain()` - rmrf the whole subdomain subtree
- **`src/resources/cache.rs`**: new module for the `pd cache clear`
  *command handler*. `pd cache clear` → `invalidate_subdomain`.
  `pd cache clear <type>` → `invalidate_type(type)`.
- **`PdClient::get_all_cursor(path, key)`**: new method for cursor-based
  pagination, used by endpoints that reject `?offset=` but support
  `?cursor=` / `next_cursor` (PD's newer convention). Replaces the
  100-item-capped `get_all_no_offset` for `/event_orchestrations` and
  `/alert_grouping_settings`.
- **`PdClient` base-URL parameterization**: refactor `send()` to accept
  an optional base URL override so `events_post()` reuses the same
  retry-on-429 and 5xx machinery without duplicating it. Events API
  v2 *does* return 429s under load; reusing the existing retry path
  is mandatory, not optional.
- **`PdClient::events_post(path, body)`**: new method on the existing
  client that targets a second base URL (`https://events.pagerduty.com`)
  with routing-key auth (body field, not header). Goes through the
  same retry machinery as `send()`. The routing key is in `body`, not
  a separate param - the caller has already inlined it.

### Data Model

Cache entry shape (one file per entry):

```
~/.cache/pd/ids/<subdomain>/<type>/<sha256(name)>.json
```

One shape:

```json
{ "version": 1, "name": "Platform", "id": "PABC123", "ts": "2026-04-16T14:02:00Z" }
```

The `name` field is stored explicitly so that cache files are
debuggable by humans and so that a read-time mismatch (someone hand-
edits the file, or a theoretical hash collision) is detectable and
treated as a miss. No negative-cache shape.

TTL: 5 minutes. Expired entries are ignored on read and overwritten on
next write.

Config additions:

- `from-email` (already present in v0.5.0)
- `routing-key` (optional escape hatch) - an explicit Events API v2
  routing key. Normally `pd change create` discovers the routing key
  dynamically via the target service's integrations (see Phase 3). The
  config key / `--routing-key` flag / `PAGERDUTY_ROUTING_KEY` env exist
  only for advanced users who already know the routing key they want
  and want to skip the lookup. If set at any layer, the dynamic lookup
  is bypassed. Default: not set.

### API Design

No new CLI commands aside from `pd cache clear [<resource-type>]` and
wiring `pd change create` (which was in the original design hierarchy).

Behavior changes:

- `pd service list app data` (two positional patterns) now issues two
  `?query=app` and `?query=data` requests (capped at 8 concurrent),
  unions results by `id`, then applies the local 3-tier filter over
  the union.
- All list filtering is case-insensitive. `pd team list platform` and
  `pd team list PLATFORM` return the same rows.
- `pd <anything> get "Name"` consults the cache before hitting the
  API. Cache is namespaced per account subdomain. Cache misses write
  negative entries so a tight loop against a misspelled name doesn't
  thrash the API.
- `pd trigger` (top-level) no longer exists. `pd incident trigger` is
  the only path.
- `pd change create --service Foo` resolves Foo's Events API v2
  integration and sends the change event with that integration's
  routing key, so the event actually lands on Foo. Advanced callers
  can still short-circuit the lookup with `--routing-key`.

### Implementation Plan

#### Phase 1: v0.5.1 bugfixes and doc drift

**Model:** sonnet

Scope: each individual fix is small and requires no new design. Several
files are touched, but every change is a targeted edit, not a new
subsystem.

**Pre-implementation verification (mandatory, blocks Phase 1 start):**

Phase 1 assumes two PD API contracts that must be confirmed against
the live tatari PagerDuty account before any code is written:

1. **`/event_orchestrations` envelope key.** Run
   `pd rest GET /event_orchestrations | jq 'keys'`. The list key is
   expected to be `event_orchestrations`. If the response uses
   `orchestrations`, the Phase 1 orchestration envelope-key change is
   already correct (the current code) and only the pagination fix
   applies.
2. **Pagination contract for `/event_orchestrations` and
   `/alert_grouping_settings`.** Run each of:
   ```
   pd rest GET /event_orchestrations?limit=1
   pd rest GET /alert_grouping_settings?limit=1
   ```
   Inspect the response for `next_cursor`, `cursor`, or a `more`
   boolean with `offset`-based continuation. The design assumes
   cursor-based pagination. If the endpoints use `offset`, replace
   `get_all_cursor` with `get_all` (which already works). If they
   use the "large-page, no continuation" pattern that
   `/incident_workflows/triggers` uses, keep `get_all_no_offset` but
   with a raised limit (100 is PD's documented cap for those endpoints).

Record the confirmed contracts in the design doc's "References"
section before proceeding. Phase 1 does not ship until both are
confirmed.

**Success criteria:** `otto ci` green. All existing 217 tests still
pass. The five new tests below pass. Both pre-implementation contracts
are confirmed and documented.

- **`PdClient::get_all_cursor(path, key)`**: new method implementing
  PD's cursor-based pagination for endpoints that accept `?cursor=`
  and return `next_cursor` in responses. Used by Phase 1 for
  orchestrations and alert grouping. No 100-item cap.
- **`src/resources/orchestration.rs`**: change envelope key from
  `"orchestrations"` to `"event_orchestrations"` in two call sites
  (`list`, `resolve_orchestration`). Switch both to `get_all_cursor`
  so results are not truncated.
  - *Verification before merge:* run `pd rest GET /event_orchestrations
    | jq 'keys'` against the tatari PD account and confirm the
    envelope key. If the API returns `orchestrations`, revert that
    part of the change and add a regression test.
- **`src/resources/grouping.rs`**: swap `get_all` for `get_all_cursor`
  in `list`.
- **`src/resources/maintenance.rs`**: rewrite `update()` as
  fetch-then-overlay-then-PUT, matching `src/resources/team.rs::update`.
- **`README.md`**: remove `--query` from `pd action list` and
  `pd incident workflow list` docs. Add Phase 4 commands
  (`maintenance`, `alert-grouping`, `orchestration`, `log`, `change`).
- **`docs/design/2026-04-16-full-api-coverage.md`**: replace "Filtering
  is done client-side via jq" with a note that `action list` uses the
  3-tier positional match. Remove deferred items that this design doc
  closes.
- **`tests/integration.rs`**: add wiremock tests for
  - `incident create` `From:` precedence: `--from` > env > config
  - `incident list --until` alone preserves default
    `statuses[]=triggered,acknowledged`
  - `client.post()` (no `From:`) does not emit a `From:` header (uses a
    custom wiremock matcher to assert absence; the positive test
    already exists in Phase 3 of the original design).
- **`tests/smoke.rs`**: assert no `--query` flag on `pd action list`
  and `pd incident workflow list` (negative assertion).

Ship as v0.5.1 (patch bump, no breaking changes).

#### Phase 2: v0.6.0 breaking cleanup and deferred design

**Model:** opus

Scope: multiple touch points, real design work for the cache, breaking
removal of the legacy trigger path.

- **Remove legacy trigger path**:
  - Delete `src/resources/trigger.rs` entirely.
  - Delete `Commands::Trigger` and `TriggerAction` from `src/cli.rs`.
  - Delete the dispatch arm and stderr warning from `src/lib.rs`.
  - Delete smoke tests targeting the top-level `pd trigger`.
  - Note the removal in `README.md` under a "Breaking changes" section.
- **Case-insensitive filter** (`src/filter.rs`):
  - Lowercase both pattern and candidate via
    `str::to_lowercase` in all three tiers.
  - Update `case_sensitive_by_default` test to assert insensitivity;
    rename it to `case_insensitive_matching`.
  - Add explicit mixed-case tests.
  - *Known limitation:* `str::to_lowercase` follows Unicode default
    casing, which does not match Turkish-locale rules for dotted/
    dotless `I`. All tatari PD resource names are ASCII, so this is
    theoretical. Document in code-comment.
- **Multi-pattern `?query` forwarding**:
  - New helper `PdClient::query_all_patterns(path, key, patterns)` on
    the existing client: given `patterns: &[String]`, issues one
    `?query=<pattern>` request per pattern through a
    `tokio::sync::Semaphore` capped at 8 concurrent requests via
    `futures::stream::iter(...).buffer_unordered(8)`, collects into a
    `Vec<Result<_>>`, bubbles the first error, unions the rest by
    `id`, and returns the deduplicated `Vec<Value>`.
  - *Failure mode:* first error surfaces; partial results are not
    returned. A user who asked for three patterns and got two silently
    would think the third had no matches. Failing loudly is correct.
  - *Concurrency cap:* explicit 8 at construction time, not "maybe
    later." Prevents PD rate-limit exhaustion on pathological inputs.
  - Flow in a resource `list` handler:
    1. `patterns.is_empty()` → existing `get_all(path)` path, no change.
    2. `patterns.len() > 0` → `query_all_patterns(path, key, patterns)`
       returns the pre-narrowed union.
    3. The existing local 3-tier filter (`filter::filter_into`) runs
       over whichever result set came back. This preserves exact /
       starts-with / contains semantics - the API query only narrows
       what the server ships us.
  - Apply to: `team.rs`, `user.rs`, `service.rs`, `escalation.rs`,
    `schedule.rs`, `maintenance.rs`. Not applicable to endpoints
    without `query` support.
- **Name-resolution ID cache** (`src/cache.rs` new module):
  - Path: `~/.cache/pd/ids/<subdomain>/<type>/<name-hash>.json`, each
    file created lazily. Root resolved via `dirs::cache_dir()`; if it
    returns `None`, cache is disabled for the run with a `tracing::
    debug!` and `resolve_*` falls through to the existing API-based
    paths. No error, no warning.
  - `<subdomain>` comes from `Config::subdomain` (already tracked;
    defaults to `"tatari"` in this repo but is overridable). Account
    namespacing prevents a staging token's IDs from ever being served
    to a production run on the same machine.
  - `<name-hash>` is the full `sha256(name)` in lowercase hex (64
    chars). Full hash eliminates any collision failure mode.
  - `--no-cache` global flag on `Cli`: forces cache bypass for the
    invocation. Useful for debugging stale-ID suspicions without
    running `pd cache clear` first.
  - Resource-type keys (directory names): `team`, `user`, `service`,
    `escalation`, `schedule`, `orchestration`, `incident-type`.
  - TTL: 5 minutes, evaluated at read time. Expired files are ignored
    on read and overwritten on next write.
  - Atomic writes: one file per entry, written via `<file>.tmp` +
    `sync_all` + rename. Two processes writing *different* names never
    contend. Two processes writing the *same* name race the rename;
    either winner produces the correct content (same name resolves to
    the same ID at one instant in time).
  - **No negative caching.** Failed lookups are not persisted. A
    negative entry with a 5-minute TTL would poison CI pipelines
    where an out-of-band process (Terraform, another `pd` session,
    the PD UI) creates a resource between a negative lookup and a
    subsequent positive lookup. PD's own retry-with-backoff handles
    the tight-loop-on-misspelling case adequately.
  - **Read flow** in a `resolve_*_id` helper:
    1. `cache::get(type, name)` → `Some(id)` means hit, return it
       without touching the API.
    2. `None` (no entry, expired, or stored `name` mismatches the
       requested `name`) → fall through to the existing API-based
       resolution. On success, `cache::put(type, name, id)`. On
       "not found", surface the error without caching.
  - **Granular invalidation**:
    - `create` handlers: on success, `cache::put(type, name, Id(new))`
      directly. No broad invalidation needed - we're adding one new
      mapping, not purging stale data.
    - `update` handlers: on success, `cache::invalidate_entry(type,
      old_name)` (the rename case - old name may now be stale) and
      `cache::put(type, new_name, Id(id))`.
    - `delete` handlers: `cache::invalidate_entry(type, name)` only.
    - `pd cache clear <type>`: `cache::invalidate_type(type)` (rmrf
      the type subdir).
    - `pd cache clear`: `cache::invalidate_subdomain()` (rmrf the
      subdomain subtree).
    - All invalidation errors (permissions, disk full) log at `warn`
      and swallow - the PD-side mutation already succeeded, and the
      5-minute TTL heals automatically.
  - **404-on-cached-id recovery**: if a `GET /<type>/<cached-id>`
    returns 404, `resolve_*_id` calls
    `cache::invalidate_entry(type, name)` and retries once against a
    fresh API call. One retry only; a second 404 surfaces the error.
  - **Known limitation: rename-without-delete staleness.** Handled
    explicitly (see below). A dedicated comment block goes at the top
    of the cache read path in `src/cache.rs`. See the "Known Cache
    Limitations" section below.
- **`pd cache clear [<resource-type>]`**:
  - No args: `cache::invalidate_subdomain()` for the current
    `Config::subdomain` - leaves other accounts' caches intact.
  - With arg: `cache::invalidate_type(<type>)` for the current
    subdomain.
  - `--all-accounts` optional flag: rmrf all of `~/.cache/pd/ids/`.
  - Command handler lives in `src/resources/cache.rs`, delegating to
    the backend in `src/cache.rs`.
- **Large comment block in `src/cache.rs`**: above the cache read
  path, include the "Rename-without-delete staleness" block (see
  "Known Cache Limitations" subsection under Technical Considerations
  below for the exact content).
- **README "Known Limitations" section**: Phase 2 adds a top-level
  `## Known Limitations` section to `README.md` that explains the
  rename-without-delete staleness to end users and tells them how to
  recover (`pd cache clear` or `--no-cache`).
- **Wire cache into resolve_\* helpers**: `team::resolve_team_id`,
  `user::resolve_user_id`, `service::resolve_service_id`,
  `escalation::resolve_escalation_id`, `schedule::resolve_schedule_id`,
  `orchestration::resolve_orchestration_id`,
  `incident::types::resolve_incident_type_id`. Maintenance windows have
  no name-to-ID resolver (they are addressed by ID); services resolved
  *within* maintenance handlers go through the cached
  `service::resolve_service_id`.
- **Tests**:
  - Cache hit/miss/expiry/invalidation unit tests in `src/cache.rs`.
  - Multi-pattern wiremock test: two `?query` requests fire, union is
    returned.
  - Case-insensitive smoke test.

Ship as v0.6.0 (minor bump; the trigger removal is breaking).

#### Phase 3: v0.7.0 Events API v2 for `pd change create`

**Model:** opus

Scope: client-layer change. Needs a second base URL, a new auth mode,
and a dynamic service-to-integration-key lookup to make `--service`
actually route events where the caller intends.

- **Base URL parameterization in `PdClient`**: refactor the private
  `send()` to accept an `Option<&str>` base-URL override. When `None`,
  uses the existing `https://api.pagerduty.com`. The 429 retry path,
  the error envelope parsing, and the tracing spans are all inherited
  unchanged. Events API calls pass `Some("https://events.pagerduty.com")`.
- **`PdClient::events_post(path, body)`**:
  - Base URL override: `"https://events.pagerduty.com"`.
  - Auth: routing key travels in the JSON body already (caller inlines
    it). No `From:` header, no Token auth header.
  - Retry: goes through the same `send()` path, so 429 and 5xx get the
    same exponential-retry treatment as the REST surface. The Events
    API v2 *does* return 429s under platform load; pretending
    otherwise risks silent change-event drops.
- **Dynamic routing-key discovery**: the critical correctness fix.
  `--service Foo` must actually cause the change event to land on
  service Foo, not on whatever service the user's global routing key
  happens to be attached to. Flow:
  1. Resolve `--service` to a service ID via the existing cached
     `service::resolve_service_id`.
  2. `GET /services/{id}?include[]=integrations`.
  3. Find an integration on the service where
     `integration.type == "events_api_v2_inbound_integration"`. Use
     its `integration_key` as the routing key for the Events API call.
  4. If no such integration exists on the service, bail with:
     "Service {name} has no Events API v2 integration. Create one
     (`pd service integration create {name} --type
     events_api_v2_inbound_integration`) or pass --routing-key
     explicitly."
- **Routing-key escape hatch**: `--routing-key` flag /
  `PAGERDUTY_ROUTING_KEY` env / config `routing-key` key. When set at
  any layer, skips the dynamic discovery and sends the event with the
  provided key. Used by advanced callers who already know the routing
  key they want (e.g. a dedicated "deploys" integration attached via
  an Event Orchestration router).
- **`src/cli.rs`**: add `ChangeAction::Create { summary, service,
  links, routing_key, from_file, example }`.
- **`src/resources/change.rs`**: handle `create`:
  - Build the PD-specified request body:
    ```json
    {
      "routing_key": "<resolved>",
      "payload": {
        "summary": "<--summary or file.summary>",
        "source": "<resolved service name>",
        "timestamp": "<RFC3339 UTC now() or file.timestamp>",
        "custom_details": { "<optional file.custom_details ...>" }
      },
      "links": [ {"href": "...", "text": "..."}, ... ]
    }
    ```
    Timestamp is always UTC (`chrono::Utc::now().to_rfc3339()`) unless
    the file provides one.
  - `--service` is required (either as flag or `file.service`). It
    drives both `payload.source` (text field) AND routing key
    resolution (real behavior).
  - `--from-file`: YAML contains payload fields (summary, source,
    custom_details, links, timestamp) only. Routing key is never in
    the file - it's a secret.
  - POST via `client.events_post("/v2/change/enqueue", body)`.
- **`src/config.rs`**: add `routing_key: Option<String>` (the escape
  hatch, not the default path).
- **`examples/change.yml`**: YAML skeleton with summary, custom_details,
  links, timestamp. Comment explains that routing key is resolved
  dynamically from `--service` and that the escape hatch is
  `PAGERDUTY_ROUTING_KEY` / config `routing-key`.
- **Tests**:
  - Wiremock for `events_post` hitting the alt base URL with the body
    shape correct and the RFC3339 timestamp stamped.
  - Wiremock for the dynamic-discovery path:
    `GET /services/{id}?include[]=integrations` returns an
    `events_api_v2_inbound_integration`, `events_post` fires with that
    integration's `integration_key` as `routing_key`.
  - Wiremock for the "no matching integration" error path.
  - Wiremock for the 429 retry (confirms the shared retry machinery
    applies to Events API calls).

Ship as v0.7.0 (minor bump).

## Alternatives Considered

### Alternative 1: Ship each fix as its own patch release

- **Description:** Fifteen patch releases, one per audit item.
- **Pros:** Easy to revert individual items; clear changelog.
- **Cons:** Ceremony overhead, and several items are interdependent
  (cache design needs the multi-pattern fix to share pattern-resolution
  helpers cleanly).
- **Why not chosen:** Phasing by blast radius (bugfix vs breaking vs
  new surface) produces fewer releases while keeping each release
  coherent.

### Alternative 2: Roll everything into one v0.6.0 release

- **Description:** Skip the v0.5.1 bugfix release; land bugs, breaking
  cleanup, cache, and Events API v2 together.
- **Pros:** Single release, no intermediate versions.
- **Cons:** The bugfixes should ship immediately because users hit them
  on first invocation; the Events API extension is unrelated surface
  area that can wait.
- **Why not chosen:** Users pay for our scheduling decisions; the bugs
  shouldn't wait for cache design to converge.

### Alternative 3: Keep `pd trigger` deprecated forever

- **Description:** Leave the stderr warning in place indefinitely.
- **Pros:** Never breaks an existing script.
- **Cons:** Nobody is running scripts against the deprecated path - the
  tool is pre-1.0 and the deprecation lived one release. Keeping it
  costs CLI surface and duplicated code in `resources/trigger.rs` and
  `resources/incident/trigger.rs`.
- **Why not chosen:** pre-1.0 is the cheap window for removals.

### Alternative 4: Implement `pd change create` via `pd rest`

- **Description:** Add a convenience wrapper that shells `pd rest POST`
  against the Events API URL, sidestepping a real client change.
- **Pros:** Less code.
- **Cons:** Doesn't actually let `pd rest` target a second base URL
  (it's hardcoded to `api.pagerduty.com`), so this isn't viable without
  the same client change.
- **Why not chosen:** the client change is unavoidable.

## Technical Considerations

### Dependencies

- `tokio`, `serde`, `serde_yaml`, `dirs` already in the dep graph.
- Phase 2 adds:
  - `futures` for `buffer_unordered` (multi-pattern helper).
  - `sha2` for the name-hash in cache filenames (`cargo add sha2`).
- Phase 3 adds `chrono` (for `Utc::now().to_rfc3339()`). Add via
  `cargo add chrono --features serde`.

### Performance

- **Multi-pattern `?query`**: parallel N requests vs single
  full-paginated scan. For a typical tatari account (dozens of
  services, handfuls of teams), two to three `?query` requests beats
  one full scan of 10+ pages.
- **Cache**: amortizes `resolve_*` latency. A tight scripted loop
  (e.g. `for svc in $(...); do pd service get "$svc"; done`) currently
  issues a `?query` list + filter per iteration. With the cache, the
  first hit populates, subsequent hits are file reads.
- **Cache file size**: negligible; each entry is ~80 bytes.

### Security

- Cache files live under `~/.cache/pd/ids/<subdomain>/...` with default
  permissions (0644). They contain PagerDuty IDs and display names,
  not secrets. Subdomain namespacing prevents staging-token IDs from
  being served on a production token's run.
- Routing key is a secret; it travels in the request body (PagerDuty's
  protocol), not logged, not written to the cache.

### Known Cache Limitations

**Rename-without-delete staleness.** This is a class of staleness the
cache cannot observe through normal mutation hooks, because the rename
happens out-of-band in the PagerDuty UI. We document it explicitly
rather than paper over it.

**Out-of-band creation after a cold lookup.** Related but distinct:
if a user hits `pd service get Foo` when Foo does not exist, the cache
stores nothing (negative caching was deliberately dropped; see Phase 2
cache section). The next invocation re-hits the API and sees Foo if
it's since been created. This is correct behavior, called out here
only to justify the "no negative caching" stance against future code
reviewers who will want to add it back.

The exact comment block to place at the top of the cache read path in
`src/cache.rs`:

```rust
// =============================================================================
// KNOWN LIMITATION: Rename-without-delete staleness
// =============================================================================
//
// When a PagerDuty resource is RENAMED via the UI (not deleted, not
// created), its ID stays stable but its name changes. The cache has no
// way to observe this rename through normal lifecycle hooks
// (create / update / delete on a given `pd` invocation) because the
// rename happened out-of-band.
//
// The failure mode: cache says "Web" -> "P0001". A user renames "Web"
// to "Web-Legacy" in the UI. The next `pd service get Web` returns the
// cached "P0001" without hitting the API. Because P0001 still exists
// as a valid service (now named "Web-Legacy"), any subsequent
// PUT/DELETE lands on the wrong service conceptually.
//
// Why we accept this:
//   1. The 5-minute TTL bounds staleness to a 5-minute window.
//   2. Verifying the rename would require a GET against the cached ID
//      on every cache hit, which defeats the purpose of the cache.
//   3. UI renames are rare compared to reads.
//
// If you suspect stale data, run `pd cache clear <type>` or pass
// `--no-cache` to bypass the cache for a single invocation.
//
// Related: we deliberately do NOT cache negative results. A cached
// "not found" with a 5-minute TTL would poison CI pipelines where an
// external system (Terraform, another `pd` session, the PD UI)
// creates the resource between two `pd` invocations. Do not add
// negative caching without revisiting this decision.
//
// =============================================================================
```

The user-facing version of this block lands in `README.md` under a
new `## Known Limitations` section in Phase 2, phrased without Rust
syntax: "If you rename a resource in the PD UI, the CLI's local
name-to-ID cache may serve the old mapping for up to 5 minutes. Run
`pd cache clear` to force a refresh, or pass `--no-cache` to bypass
for one command."

### Testing Strategy

- **Phase 1**: wiremock tests for request shape and handler behavior.
  Cursor pagination unit test (2-page response, confirms full results
  assembled). `From:`-header precedence tests. `--until`-alone default
  statuses test.
- **Phase 2 cache**: unit tests for hit/miss/expiry/granular
  invalidate/subdomain namespacing/hash-collision name mismatch. A
  smoke test for `pd cache clear` and `pd cache clear <type>`.
- **Phase 2 multi-pattern**: wiremock test asserting N `?query`
  requests fire (N = pattern count, capped at 8) and the union by
  `id` is returned deduplicated.
- **Phase 2 case-insensitive**: filter unit tests with mixed case.
- **Phase 3 Events API**:
  - Wiremock at the alt base URL; client is base-URL-overridable via
    the new `send()` parameterization.
  - Dynamic-discovery path: mock `GET /services/{id}?include[]=
    integrations` returning an `events_api_v2_inbound_integration`;
    confirm `events_post` fires with that integration's key.
  - "No matching integration" path: service has only a
    `generic_events_api_inbound_integration`; confirm the error
    message points the user at the right recovery.
  - 429 retry: confirm events calls share the REST retry machinery.

### Rollout Plan

- v0.5.1: patch bump, ships Phase 1. `cargo install --path .` +
  `bump --message` commit.
- v0.6.0: minor bump, ships Phase 2. Add a `## Breaking changes`
  section at the top of `README.md` calling out the `pd trigger`
  removal. No separate `CHANGELOG.md` - this repo uses README +
  design docs for change history.
- v0.7.0: minor bump, ships Phase 3.
- Each phase goes through the same PR + install loop Phases 1-4 of the
  original design used.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `event_orchestrations` envelope key is `orchestrations` not `event_orchestrations` | Low | Low | Confirmed against live API during Phase 1 pre-implementation verification |
| Cursor pagination contract is not what we assumed | Low | High | Confirmed against live API during Phase 1 pre-implementation verification; fallback to `get_all` or `get_all_no_offset` if cursor isn't the model |
| Cache cross-account pollution (staging vs prod on the same laptop) | ~~High~~ Resolved | ~~High~~ | Cache path is namespaced by `Config::subdomain`; a staging run and a prod run cannot collide |
| Lost updates from concurrent writers to the same cache file | ~~Med~~ Resolved | ~~Med~~ | Per-entry files eliminate the read-modify-rewrite race; concurrent writers to different names never touch the same file |
| Negative-cache poisoning (CI creates resource after cold lookup) | ~~High~~ Resolved | ~~High~~ | Negative caching dropped entirely. PD retry-with-backoff absorbs misspelling loops |
| Hash-collision cache thrashing | ~~Low~~ Resolved | ~~Low~~ | Full `sha256(name)` (64 hex chars) in filenames. Birthday bound for collisions is 2^128 names |
| Rename-without-delete cache staleness | Med | Low | Accepted limitation: documented in code, design doc, and README. Bounded by 5-min TTL. `pd cache clear` / `--no-cache` recover |
| Multi-pattern `?query` bursts exhaust rate limits | Low | Med | `tokio::sync::Semaphore(8)` cap inside `query_all_patterns`. Existing 429 retry absorbs what the cap lets through |
| Events API v2 returns 429s we don't retry | ~~Med~~ Resolved | ~~Med~~ | `events_post` shares the `send()` retry path via base-URL parameterization; no separate retry code |
| `--service` routes change events to the wrong service | ~~High~~ Resolved | ~~High~~ | Dynamic lookup of the service's `events_api_v2_inbound_integration` replaces global routing-key assumption. Escape hatch preserved for advanced callers |
| `pd trigger` removal breaks scripts | Low | Low | Pre-1.0; stderr deprecation shipped one release ago; call out in README breaking-changes |

## Open Questions

Most original Open Questions were promoted to Phase 1's
pre-implementation verification (envelope key, pagination contract).
Remaining:

- [ ] Does the cache need a `pd cache warm` command to pre-populate?
  Not planned; users warm it organically through normal use. Revisit
  if a script-heavy workflow surfaces a cold-start pain point.
- [ ] Events API change endpoint integration type: confirm the exact
  value (`events_api_v2_inbound_integration` vs a variant). Prototype
  with `curl` plus a real integration before Phase 3. Same pattern as
  Phase 1's pre-implementation verification, applied to Phase 3.

### Definition of Done

This design doc is marked `**Status:** Implemented` when all three of
the following are true:

1. v0.5.1, v0.6.0, and v0.7.0 have shipped with their respective
   phase content.
2. The original full-API-coverage design doc
   (`docs/design/2026-04-16-full-api-coverage.md`) has had its "Open
   items carried forward from Phase N" sections rewritten to remove
   every item this doc addresses.
3. `otto ci` is green on `main` and `pd v0.7.0` is installed locally.

## References

- `docs/design/2026-04-16-full-api-coverage.md` - the original design
  doc this one closes gaps against.
- Gemini Architect audit from 2026-04-16 (conversation transcript).
- PagerDuty Events API v2 docs:
  https://developer.pagerduty.com/docs/events-api-v2/overview/

### Phase 1 pre-implementation verification results (2026-04-16)

Confirmed against the live tatari PagerDuty account before Phase 1 code
was written:

- **`/event_orchestrations` envelope key:** `orchestrations` (NOT
  `event_orchestrations`). The existing code is correct; the audit item
  to rename the envelope key dissolves. No change in
  `src/resources/orchestration.rs` for this item.
- **`/event_orchestrations` pagination:** offset-based
  (`limit` + `offset` + `more` + `total`). The existing `get_all`
  already handles this correctly. No change required; the original
  shakedown item assumed cursor pagination and was mistaken about this
  endpoint.
- **`/alert_grouping_settings` pagination:** cursor-based using
  `after` / `before` cursors in both request and response. An `after`
  of `null` in the response signals the final page. This does NOT
  match the design doc's `next_cursor` assumption. The new
  `PdClient::get_all_cursor(path, key)` helper uses `after` as the
  query param and response field name. Swap `get_all` →
  `get_all_cursor` in `src/resources/grouping.rs::list` only; leave
  orchestration on `get_all`.

These findings reduce Phase 1's envelope/pagination scope to a single
endpoint (alert-grouping) and a cursor helper whose contract is
`after`-based, not `next_cursor`-based.

### Phase 3 pre-implementation verification results (2026-04-16)

Ran `GET /services?include[]=integrations&limit=100` against the live
tatari account and enumerated the distinct `integrations[].type`
values. Two types appear in the fleet:

- `events_api_v2_inbound_integration` - the type the Events API v2
  change endpoint expects. Used by `pd change create` for dynamic
  routing-key discovery.
- `generic_events_api_inbound_integration` - the legacy Events API v1
  integration. Services with only this type must either add an
  Events API v2 integration or pass `--routing-key` explicitly.

The matcher string in `resources::change::create` is the verified
literal `"events_api_v2_inbound_integration"`.
