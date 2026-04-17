# Design Document: Shakedown of v0.6.4

**Author:** Scott Idler
**Date:** 2026-04-17
**Status:** In Review
**Review Passes Completed:** 5/5

## Summary

Close the three bugs surfaced by the v0.6.4 shakedown
(`docs/shakedown-v0.6.4.md`) and ship the result as v0.6.5. Add the six
missing table renderers, drop the unresolvable EMAIL column from
`team member list`, and make `schedule override list` default to a
sensible time window so it stops returning an unhelpful 400. Before any
of that, diagnose and restore the Release workflow - which has been
silently failing to fire on tag pushes since v0.2.1 - so the fix
actually reaches users.

## Problem Statement

### Background

`docs/shakedown-v0.6.4.md` ran 58 read-only commands against the live
PagerDuty account. Zero commands crashed. Three user-visible bugs
surfaced, plus a release-pipeline gap:

- `--output table` silently falls back to JSON for six list commands
  (`incident`, `orchestration`, `alert-grouping`, `change`, `log`,
  `maintenance`). `output/table.rs::render()` has no match arm for any
  of those envelope keys.
- `team member list --output table` always shows a blank EMAIL column
  because the `/teams/{id}/members` endpoint returns `user` as a
  reference object (`{id, summary, type}`) without an email field.
- `schedule override list "SRE Schedule"` with no dates returns
  `400 Bad Request: Since cannot be empty`. The CLI marks `--since`
  and `--until` as optional but the API does not.
- v0.3.0 through v0.6.4 exist as annotated git tags on origin but the
  Release workflow (`.github/workflows/release.yml`) has **zero** runs
  recorded for any of them. Per `gh run list --workflow=release.yml`,
  only v0.2.0 and v0.2.1 ever fired the pipeline. Tag objects are all
  annotated (confirmed via `git cat-file -t`), the workflow file is
  present at each tag's tree, and the workflow is active. Something
  about how the last eight tags were pushed prevented the `push: tags:
  v*` trigger from firing.

### Problem

The three code bugs are individually cosmetic or narrow UX issues, but
together they chip at the promises the CLI makes:

- **Silent JSON fallback** on six list commands undermines
  "`--output table` works on every list." A user who sets the flag
  gets JSON and sees nothing wrong until they try to eyeball the
  output. Tables for the commands users reach for most
  (`incident list`, `log list`, `orchestration list`) simply do not
  render.
- **Blank EMAIL column** on `team member list` is worse than no
  column: the table actively shows a field labeled `EMAIL` that is
  never populated. It looks like a bug every time, because it is.
- **`schedule override list` 400 on common use** is a UX bug: the
  interactive case "what are the overrides on this schedule right
  now?" fails with an opaque API error.

The release-pipeline gap is the outer bug - even after fixing the
three code bugs, the fix cannot reach anyone installing from GitHub
releases while the most recent published binary is four minor
versions behind and eight consecutive tag pushes have failed to
trigger a release.

### Goals

- Every current list command renders a real table under `--output table`.
- No list command renders a column whose data is known to be
  structurally absent.
- `schedule override list` works with no arguments by defaulting to a
  reasonable window, and the error on API rejection (if a user passes
  impossible values) still surfaces clearly.
- v0.6.5 is published on GitHub releases with all four platform
  binaries and SHA256 checksums.

### Non-Goals

- Not addressing table-renderer coverage for single-resource `get`
  responses; those already fall back to JSON by design and there is
  no demand for tabularizing a one-row object.
- Not hydrating `team member list` with N+1 `/users/{id}` calls to
  recover emails. The shakedown documented the workaround
  (`pd user get <ID>`) and the cost/benefit does not justify it.
- Not rewriting `client::get_all` pagination or changing any resource
  fetcher beyond the override handler.
- Not publishing backfill GitHub releases for v0.3.0 - v0.6.4. The
  release pipeline only needs to fire for v0.6.5 forward. The old tags
  stay as-is.

## Proposed Solution

### Overview

Three small, independent code changes plus a release-pipeline
diagnosis that must land first:

0. **Diagnose why the Release workflow has not fired since v0.2.1.**
   Determine the exact mechanism (local git config, token lacking
   `workflows` scope, `gh api`-created refs, batched push refs not
   triggering tag events, etc.). Prove the fix by adding a
   `workflow_dispatch:` trigger to `release.yml` first - dispatching
   it manually from the Actions UI or `gh workflow run` does not
   require a tag push and exercises the same build + publish steps -
   then verify the automatic `push: tags: v*` trigger separately on
   the v0.6.5 tag push in Phase 4.
1. **Six new renderer arms in `output/table.rs`.** Each arm chooses
   2-5 informative columns using the existing `render_table` helper.
2. **Drop the EMAIL column from `render_members`.** Final layout is
   three columns: `USER_ID`, `NAME`, `ROLE`. No replacement column is
   added; the user-reference object does not carry any other field
   worth showing.
3. **Default `--since` / `--until` in `schedule override list`.** Use
   module-level constants for the window (now - 7 days, now + 30 days)
   resolved at the handler level. The CLI flags stay optional; users
   who want a different window pass explicit values.
4. **Publish v0.6.5 once Phase 0 is green.** Bump, tag, push, verify
   the release lands with all four platform binaries.

### Architecture

No architectural changes. Code work touches three files:

```
src/output/table.rs              # six new renderer functions + dispatch arms
                                 # EMAIL drop on render_members
                                 # i64_field helper
src/resources/schedule.rs        # default window in override_list + consts
src/cli.rs                       # doc-comment defaults on override_list flags
```

Phase 0 (release pipeline) may also touch
`.github/workflows/release.yml` (to add `workflow_dispatch:`) or
result in no file change at all if the fix is operational
(credential / push-mechanism).

### Data Model

No new types. Envelope keys the new renderers dispatch on, and the
columns chosen for each:

| Envelope key              | Columns (ordered)                                       |
|---------------------------|---------------------------------------------------------|
| `incidents`               | `#`, `STATUS`, `URGENCY`, `PRIORITY`, `SERVICE`, `TITLE` |
| `orchestrations`          | `ID`, `NAME`, `TEAM`, `ROUTES`                          |
| `alert_grouping_settings` | `ID`, `NAME`, `TYPE`, `DESCRIPTION`                     |
| `change_events`           | `ID`, `TIMESTAMP`, `SOURCE`, `SUMMARY`                  |
| `log_entries`             | `ID`, `CREATED`, `TYPE`, `AGENT`, `SUMMARY`             |
| `maintenance_windows`     | `ID`, `START`, `END`, `SERVICES`, `DESCRIPTION`         |

Column choices follow the same rules the existing renderers use:

- ID-like short fields come first so they line up on the left.
- Numeric/enum fields sit next to the ID and stay narrow.
- Long free-text fields (titles, summaries, descriptions) come last so
  the `render_table` widest-column shrinker truncates them first.
- Counts render the same way `render_escalations` does for `RULES`:
  `arr.len().to_string()` when the source field is an array. Note that
  `/event_orchestrations` returns `routes` as an integer count directly
  (not an array), so `ROUTES` extracts it with `i64_field` rather than
  counting an array. `SERVICES` on `alert_grouping_settings` and
  `maintenance_windows` is an array and does get counted.

For `render_members`, the EMAIL column is removed and the remaining
columns become `USER_ID`, `NAME`, `ROLE` (three columns, as before
minus EMAIL).

Per-renderer field extractors (one per column, in display order):

```
render_incidents:
  #        -> i64_field(r, "incident_number")
  STATUS   -> str_field(r, "status")
  URGENCY  -> str_field(r, "urgency")
  PRIORITY -> nested_str(r, "priority", "summary")   # blank when null
  SERVICE  -> nested_str(r, "service", "summary")
  TITLE    -> str_field(r, "title")

render_orchestrations:
  ID     -> str_field(r, "id")
  NAME   -> str_field(r, "name")
  TEAM   -> nested_str(r, "team", "summary")
  ROUTES -> i64_field(r, "routes")                    # integer count

render_alert_grouping:
  ID          -> str_field(r, "id")
  NAME        -> str_field(r, "name")
  TYPE        -> str_field(r, "type")
  DESCRIPTION -> str_field(r, "description")

render_change_events:
  ID        -> str_field(r, "id")
  TIMESTAMP -> str_field(r, "timestamp")
  SOURCE    -> str_field(r, "source")
  SUMMARY   -> str_field(r, "summary")

render_log_entries:
  ID      -> str_field(r, "id")
  CREATED -> str_field(r, "created_at")
  TYPE    -> str_field(r, "type")
  AGENT   -> nested_str(r, "agent", "summary")
  SUMMARY -> str_field(r, "summary")

render_maintenance_windows:
  ID          -> str_field(r, "id")
  START       -> str_field(r, "start_time")
  END         -> str_field(r, "end_time")
  SERVICES    -> services array length (same pattern as
                 render_escalations "RULES")
  DESCRIPTION -> str_field(r, "description")

render_members (updated):
  USER_ID -> nested_str(r, "user", "id")
  NAME    -> nested_str(r, "user", "summary")
  ROLE    -> str_field(r, "role")
```

`i64_field` is the one small helper to add alongside `bool_field`:

```rust
fn i64_field(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(|v| v.as_i64())
        .map(|n| n.to_string())
        .unwrap_or_default()
}
```

### API Design

No changes to CLI flag shapes. `schedule override list` keeps
`--since` and `--until` optional, documented as:

```
--since  ISO-8601 lower bound [default: 7 days ago]
--until  ISO-8601 upper bound [default: 30 days from now]
```

Module-level constants in `src/resources/schedule.rs`:

```rust
const OVERRIDE_DEFAULT_SINCE_DAYS: i64 = 7;
const OVERRIDE_DEFAULT_UNTIL_DAYS: i64 = 30;
```

Applied in `override_list`:

```rust
let since = since.map(str::to_string).unwrap_or_else(|| {
    (Utc::now() - Duration::days(OVERRIDE_DEFAULT_SINCE_DAYS)).to_rfc3339()
});
let until = until.map(str::to_string).unwrap_or_else(|| {
    (Utc::now() + Duration::days(OVERRIDE_DEFAULT_UNTIL_DAYS)).to_rfc3339()
});
```

`chrono` is already a direct dependency (`Cargo.toml` pins
`chrono 0.4.44`, used by `src/resources/change.rs` for Events API v2
timestamps). No `cargo add` needed.

### Implementation Plan

#### Phase 0: Restore a working release path
**Model:** opus

The gate for Phases 1-3 is "we have a reliable way to publish v0.6.5
when the code changes are ready." The guaranteed way to reach that
gate is to add a `workflow_dispatch:` trigger; diagnosing the
`push: tags` failure is additive.

- **Required:** Add a `workflow_dispatch:` trigger to
  `.github/workflows/release.yml` with a string input `tag` that is
  used both as the checkout ref (`actions/checkout@v4` `ref: tag`)
  and as the release tag (`softprops/action-gh-release@v2` `tag_name:
  tag`, also override `github.ref_name` references in the existing
  artifact names to use `${{ inputs.tag }}`). This lets a release be
  published from any existing annotated tag via
  `gh workflow run release.yml --ref main -f tag=vX.Y.Z` and does not
  depend on the broken `push: tags` trigger.
- **Required:** Smoke-test the dispatch trigger by publishing a
  release from an existing tag. Candidate: run it with `tag=v0.6.4`
  to publish the current release rather than waiting on v0.6.5.
  Confirm all four platform binaries and SHA256 files land on the
  v0.6.4 release.
- **Optional, same phase:** Diagnose why `push: tags: v*` has not
  fired since v0.2.1. Cross-check
  `gh run list --workflow=release.yml` against
  `git ls-remote --tags origin`; look at local git config
  (`push.followtags=true`, `branch.main.pushremote=no_push` pointing
  at an undefined remote); verify the credential used to push tags
  has `workflows` scope. If the fix is obvious (e.g., the shipping
  flow uses a restricted token; swap for a PAT with Actions write)
  apply it. If not, document what was ruled out and leave the
  dispatch trigger as the operating mode.
- Document the manual-dispatch command in the repo README so
  future releases are not gated on rediscovering it.

#### Phase 1: Drop EMAIL column from `team member list`
**Model:** sonnet

- Update `render_members` in `src/output/table.rs` to the three-column
  layout: `USER_ID`, `NAME`, `ROLE`.
- Add a unit test in `table.rs` asserting: the rendered output contains
  `USER_ID`, `NAME`, `ROLE`; does **not** contain the literal `EMAIL`;
  and a sample member row (with the user as a reference object
  `{id, summary, type}`) renders all three columns populated.
- `otto ci` passes.

#### Phase 2: Add six missing table renderers
**Model:** sonnet

- Add six `render_*` functions to `src/output/table.rs` following the
  shape of existing renderers (`render_users`, `render_escalations`).
- Add the `i64_field` helper alongside `bool_field`.
- Wire each into the dispatch in `render()` in the same order as the
  table in "Data Model" above.
- Add one unit test per renderer asserting header presence and a
  sample row.
- Add these specific edge-case tests - these are the shapes the live
  API actually returns:
  - `render_incidents` with `priority: null`: assert the row renders,
    no `"null"` string appears.
  - `render_log_entries` with a log entry that has no `agent` field:
    assert the row renders, no `"null"` string appears.
  - `render_alert_grouping` with `description: null`: assert the row
    renders, no `"null"` string appears.
  - `render_maintenance_windows` with an empty `services` array:
    assert the SERVICES column renders `0`, not blank.
- Add one empty-list test for `render_incidents` (the most common
  empty case, see the shakedown where `incident list` returned zero
  rows by default).
- `otto ci` passes.

**Data-shape verification step before writing tests:** run each of the
six commands against the live account with parameters that return
rows and capture one JSON sample:

```
pd --output json incident list --status resolved --since 2026-04-01 | jq '.incidents[0]' > /tmp/sample-incident.json
pd --output json orchestration list | jq '.orchestrations[0]' > /tmp/sample-orchestration.json
pd --output json alert-grouping list | jq '.alert_grouping_settings[0]' > /tmp/sample-grouping.json
pd --output json change list | jq '.change_events[0]' > /tmp/sample-change.json
pd --output json log list | jq '.log_entries[0]' > /tmp/sample-log.json
pd --output json maintenance list | jq '.maintenance_windows[0]' > /tmp/sample-maintenance.json
```

If any of those return an empty array (the Tatari account had no
`change_events` or `maintenance_windows` at shakedown time), use a
`rest GET` with a broader filter or fall back to the PD API reference
shape. Update the Data Model extractor table in this doc if an
observed field name differs from what is listed.

#### Phase 3: Default `--since` / `--until` for schedule override list
**Model:** sonnet

- Add `OVERRIDE_DEFAULT_SINCE_DAYS` / `OVERRIDE_DEFAULT_UNTIL_DAYS`
  consts at the top of `src/resources/schedule.rs`.
- Rewrite `override_list` to apply the defaults before building the
  query string.
- Update the `--help` text on the `List` variant in `cli.rs`. The
  defaults are relative-to-now and cannot be expressed as a clap
  `default_value` string literal, so they live in the doc-comment on
  `ScheduleOverrideAction::List::since` and `::until`:

  ```rust
  /// ISO-8601 lower bound (default: 7 days ago)
  #[arg(long)]
  since: Option<String>,
  /// ISO-8601 upper bound (default: 30 days from now)
  #[arg(long)]
  until: Option<String>,
  ```

  Clap renders the doc-comment under the flag description; this is the
  mechanism other commands in this repo already use for relative
  defaults.
- Add a wiremock integration test (`tests/integration.rs` already uses
  wiremock - mirror its existing pattern) asserting that
  `pd schedule override list "..."` with no `--since` / `--until`
  issues a request whose query string contains `since=` and `until=`,
  and that the resolved values are valid RFC 3339 timestamps
  (regex-check the trailing `Z` or `+00:00`, do not compare to exact
  timestamps).
- `otto ci` passes.

#### Phase 4: Ship v0.6.5
**Model:** sonnet

- `bump` (patch) to bring Cargo.toml from 0.6.4 -> 0.6.5.
- Push commit + tag to origin via the shipping flow (`shipit` skill).
- Publish the release:
  - If the `push: tags` trigger was restored in Phase 0, the Release
    workflow fires automatically; watch it complete.
  - Otherwise, invoke the dispatch trigger:
    `gh workflow run release.yml --ref main -f tag=v0.6.5`.
- Confirm all four platform binaries
  (`linux-amd64`, `linux-arm64`, `macos-arm64`, `macos-x86_64`) plus
  matching SHA256 files appear under the v0.6.5 GitHub release.
- `cargo install --path .` locally to update the dev binary.
- Re-run the three failing scenarios from the shakedown against the
  installed binary:
  - `pd --output table incident list --status resolved --since 2026-04-01`
    renders a table.
  - `pd --output table team member list "SRE"` shows three columns, no
    EMAIL header.
  - `pd schedule override list "SRE Schedule"` succeeds without
    `--since`.
- Record the results in a short addendum at the bottom of
  `docs/shakedown-v0.6.4.md`.

## Alternatives Considered

### Alternative 1: Hydrate emails in `team member list` via N+1 lookups

- **Description:** Issue one `/users/{id}` GET per member to populate
  the EMAIL column. Parallelize with a semaphore.
- **Pros:** Preserves the column; users do not need to chain commands.
- **Cons:** A team with 30 members turns one API call into 31. Against
  the live API this is noticeably slow and burns rate-limit budget for
  a column that is rarely load-bearing at the table-rendering level.
  Also moves the renderer's contract from "format what the resource
  handed me" to "fetch more data," which is a fundamentally different
  seam.
- **Why not chosen:** Wrong layer. Hydration is a resource-handler
  concern, not an output-formatter concern. If hydration is ever
  warranted it belongs in `resources::team::member_list`, gated by a
  flag like `--hydrate` or `--with-email`.

### Alternative 2: Mark `--since` as required on `schedule override list`

- **Description:** Change `since: Option<String>` to `since: String`
  in `ScheduleOverrideAction::List`. Clap enforces it, and the error
  surfaces at parse time with a useful message.
- **Pros:** Simple, explicit, mirrors the underlying API contract.
- **Cons:** The interactive common case - "what are the overrides on
  this schedule right now?" - becomes three keystrokes longer every
  time. PD's web UI defaults to a 14-day window; aligning with that
  shape is friendlier than forcing the user to compute an ISO-8601
  timestamp.
- **Why not chosen:** Optimizes for API fidelity at the cost of the
  interactive UX the CLI is actually used for.

### Alternative 3: Surface a custom error instead of defaulting

- **Description:** Detect the missing `since` before calling the API
  and return `bail!("Provide --since (e.g., --since 2026-01-01)...")`.
- **Pros:** Zero ambiguity about what the CLI is sending.
- **Cons:** Every first use of the command is still a failure. It is a
  strictly worse version of Alternative 2 - same friction, no parse-time
  guarantee.
- **Why not chosen:** Does not meet the goal of "works with no
  arguments."

### Alternative 4: Defer the six missing renderers and ship renderers incrementally

- **Description:** Pick two or three of the highest-value missing
  renderers (say `incidents`, `change_events`, `log_entries`) and punt
  the rest.
- **Pros:** Smaller diff, smaller test surface.
- **Cons:** The "silent JSON fallback" pattern is the actual problem;
  every list command that does not render a table erodes the
  `--output table` contract a little more. Shipping three-of-six leaves
  the footgun armed for three more endpoints.
- **Why not chosen:** The renderer functions are each ~15 lines, the
  tests are small, and landing them together is the only way to say
  "every list has a table" without caveats.

## Technical Considerations

### Dependencies

- `chrono` for timestamp math in the override window defaults. Already
  a direct dependency (`Cargo.toml` pins `chrono 0.4.44`, already used
  by `src/resources/change.rs` for Events API v2 timestamp formatting).
- No other new dependencies. A small `i64_field` helper parallels the
  existing `bool_field` in `src/output/table.rs`; one-function
  addition, no new crate.

### Performance

- The six new renderers each iterate `rows.len()` times, same as the
  existing fourteen. No pagination or network changes. Cost is O(rows)
  formatting on an already-fetched response.
- The override-window defaults do not change the number of API calls -
  one GET per invocation, same as today.

### Security

- None. No new surface, no credentials, no user-controlled pathing or
  shell-outs.

### Testing Strategy

The per-phase test lists under "Implementation Plan" are the
authoritative checklist. Cross-cutting notes:

- Prefer wiremock for any test that asserts an outbound request
  shape. `tests/integration.rs` already uses wiremock for other
  handlers; mirror that pattern for the override defaults test.
- No smoke-test changes are needed; the new behavior does not change
  the JSON envelope of any command.
- Post-release verification (on the live Tatari account, in Phase 4)
  is the last required test before declaring the bug closed.

### Rollout Plan

- Phase 0 unblocks everything. Its completion criterion is a
  successful dispatch publish of v0.6.4 (proving the pipeline itself
  works end-to-end), regardless of whether the `push: tags` trigger
  is restored in the same phase.
- Single patch-level release: v0.6.4 -> v0.6.5 covers all three code
  fixes. Use `bump` (patch) to update Cargo.toml; do not edit the
  version by hand. Per repo policy, tags are annotated and only
  created on `main`.
- Validate on the live Tatari account with the three scenarios
  listed in Phase 4 before declaring the release done.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Renderer picks wrong column for a shape not seen in the Tatari account (e.g., `alert_grouping_settings` variant type we never exercised) | Medium | Low | Renderer reads fields defensively (missing -> blank cell), matches the pattern of every other renderer. Add tests for at least one non-obvious variant per shape. |
| Override window default hides rows a user actually wanted (e.g., override scheduled 60 days out) | Medium | Low | Document the defaults in `--help`. Power users who know the API can always pass `--since`/`--until` explicitly. |
| `chrono` timestamp formatting mismatch (e.g., omitting `Z` for UTC) breaks the PD API filter | Low | Medium | Use `Utc::now().to_rfc3339()`, matching the `src/resources/change.rs` precedent that PD already accepts. Cover with a unit or wiremock test asserting the query string contains a `Z` or `+00:00` suffix. |
| Release pipeline fails to fire for v0.6.5, extending the "four versions behind" gap to five | Medium | High | This is not a theoretical risk - it has happened eight consecutive times. Phase 0 gates Phase 4 specifically because of it. If the automatic trigger cannot be restored in-phase, fall back to `gh workflow run release.yml --ref v0.6.5` and document that workaround in the README so it is not forgotten next release. |
| PD API's maximum allowed window on `/schedules/{id}/overrides` differs from our 37-day default (7 back + 30 forward) | Low | Low | The PD API docs do not cite a hard ceiling; 37 days is small. If the API rejects the default window in some account, the 400 error surfaces with a clear message and the user can pass explicit bounds. |
| Override default uses local `chrono::Utc::now()` but the PD API interprets timestamps in the schedule's time zone | Low | Low | The API accepts any RFC 3339 timestamp with explicit offset and normalizes internally. `Utc::now().to_rfc3339()` produces a `+00:00` offset, which is unambiguous. `src/resources/change.rs` uses the same pattern against the Events API without issue. |
| EMAIL drop surprises a downstream `jq` pipeline | Low | Low | The drop is only in the table renderer. JSON output is unchanged. Any scripted pipeline is already going through `--output json`. |

## Open Questions

None. Decisions made during Pass 4:

- **ASSIGNED column on `render_incidents`: no.** `assignments` is
  frequently empty (resolved incidents with no assignee on the final
  state), carries a long summary string that eats width budget, and
  the incident list is already at six columns which is the widest of
  the new renderers. Users who want the assignee have `--output json`
  and `incident get`. Revisit if a scripting pattern emerges that
  needs it at the list level.
- **Override window defaults configurable via `pagerduty-cli.yml`:
  no.** Per rules/rust.md, expose config only when values are
  user-tunable at a scale where CLI flags are insufficient. Override
  windows are already CLI-overridable via `--since` / `--until`. No
  other relative-time default in the CLI is configured from YAML; we
  are not starting that pattern on this bug. Revisit if someone files
  a request.

## References

- `docs/shakedown-v0.6.4.md` - source shakedown report
- `docs/design/2026-04-16-shakedown-fixes.md` - v0.1.0 shakedown
  resolution (prior art for this doc's shape)
- `docs/design/2026-04-16-shakedown-v0.5.0.md` - v0.5.0 shakedown
  resolution (more recent prior art)
- `src/output/table.rs` - the renderer dispatcher and fourteen existing
  arms being extended
- PagerDuty API `/schedules/{id}/overrides` reference for `since`
  semantics
