# Design Document: pd v0.1.0 Shakedown Fixes

**Author:** Scott Idler
**Date:** 2026-04-16
**Status:** Draft
**Review Passes Completed:** 5/5

## Summary

Fix every gap surfaced by the v0.1.0 shakedown: two commands returning 400,
a display-name lookup that silently fails, an unimplemented table renderer, a
sample config that doesn't reflect real fields, and a workflow export that
returns empty data. Land all fixes plus regression tests under a single `v0.2.0`
release.

## Problem Statement

### Background

The v0.1.0 shakedown against the real PagerDuty API (`docs/shakedown-v0.1.0.md`)
exercised every command in the tree, found two outright failures, one UX
regression, one no-op, one misleading stub, and one API-modeling surprise. The
tool is otherwise sound, but these gaps undermine trust in the binary for the
two things it will actually be used for day-to-day: listing triggers/actions
and round-tripping workflow definitions through YAML.

### Problem

Six distinct defects, ranked by severity:

1. **`pd trigger list` and `pd action list` return HTTP 400.** Both endpoints
   accept `?limit=N` but reject `?offset=0`. The `client::get_all` helper
   unconditionally appends both, so these commands can never succeed against
   the real API.
2. **`pd incident-type get "Managed Incident"` returns HTTP 404.** Clap help
   advertises `<ID_OR_NAME>`, but the handler passes the argument straight to
   `/incidents/types/{id_or_name}`, which only resolves by slug or ID. Display
   names silently don't work. `trigger.rs` already had a display-name resolver
   and then removed it after discovering the API accepts display names inline
   for *that* endpoint; the same courtesy is missing here.
3. **`pd --output table` is a no-op.** The source explicitly falls back to
   pretty JSON with a "Phase 1" comment. Users who set `table` get JSON.
4. **`pagerduty-cli.yml` ships a fake config.** The example file has `name`,
   `age`, `debug` fields that no code path reads. A new user copying this will
   not get a working config.
5. **`pd incident-workflow export` always returns `trigger: null` and empty
   `steps: []`** for the two workflows currently in the Tatari account. Live
   investigation showed `/incident_workflows` lists *stub* workflow IDs
   (`PL7CP1H`, `PTFBRY3`) that are empty shells; the actual workflow data with
   steps and triggers lives under different IDs (`PHYROPU`, `PQ6QQID`) that are
   **not** in the list response. This is a PagerDuty API behavior, not a CLI
   bug, but the CLI currently has no way to work around it, so `export` is
   effectively broken for these workflows.
6. **Secrets repo renamed mid-shakedown.** `pagerduty-api-key.age` became
   `pagerduty-api-token.age` to match the env var the tool reads
   (`PAGERDUTY_API_TOKEN`). Already fixed; just needs documenting in the
   README auth section.

### Goals

- `pd trigger list` and `pd action list` return 200 and print their data
- `pd incident-type get "Managed Incident"` works exactly like `pd incident-type get managed_incident`
- `--output table` renders a genuine table for the list commands; falls back
  to JSON for commands where a table makes no sense
- `pagerduty-cli.yml` documents only the real config fields
- `pd incident-workflow export` returns the real steps and trigger, even when
  the PagerDuty API splits the workflow across stub and implementation IDs
- README auth section matches the actual env var (`PAGERDUTY_API_TOKEN`)
- All fixes ship with regression tests that would have caught the original bugs
- Tagged and released as `v0.2.0`

### Non-Goals

- Rewriting the pagination layer into a generic cursor abstraction
- Building full table rendering for every command (only list commands)
- Supporting display-name lookup for commands other than `incident-type get`
  (the only one users hit this against)
- Fixing PagerDuty's stub/implementation workflow ID split as a general
  problem; scoped to making `export` produce usable YAML

## Proposed Solution

### Overview

Six targeted fixes in one release. Most are independent; Phase 4 (workflow
export) depends on Phase 1 (pagination helper). Add a regression test per fix.

### Fix 1: Pagination drops `offset=0` on endpoints that reject it

Two endpoints don't support offset-based pagination: `/incident_workflows/triggers`
and `/incident_workflows/actions`. Both accept `?limit=N` alone but reject
`?limit=N&offset=0`. Rather than overhauling `get_all`, add a sibling method
`get_all_no_offset` that appends only `limit` and does a single-page fetch
(both endpoints return all results in one response at the max page size).

```rust
// src/client.rs
const LARGE_PAGE_LIMIT: u32 = 200; // use a larger limit since we can't paginate

pub async fn get_all_no_offset(&self, path: &str, key: &str) -> Result<Vec<Value>> {
    let sep = if path.contains('?') { '&' } else { '?' };
    let paginated = format!("{}{}limit={}", path, sep, LARGE_PAGE_LIMIT);
    let resp = self.get(&paginated).await?;
    // Warn loudly if the response says there's more - we can't fetch it without offset.
    if resp.get("more").and_then(|v| v.as_bool()).unwrap_or(false) {
        tracing::warn!(
            path = %path,
            "endpoint reports more=true but doesn't support offset pagination; \
             results are truncated at {}. File a bug if this is hit in production.",
            LARGE_PAGE_LIMIT
        );
    }
    Ok(resp.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default())
}
```

Update `src/resources/trigger.rs::list` and `src/resources/action.rs::list` to
call `get_all_no_offset` instead of `get_all`.

Regression test: mock `/incident_workflows/triggers?limit=200` returning 200
with 3 triggers and `/incident_workflows/triggers?limit=25&offset=0` returning
400; assert the command succeeds using the 200-limit endpoint.

### Fix 2: `incident-type get` resolves display names

Add a new client method `try_get` that returns `Ok(None)` on HTTP 404 and
propagates every other error. String-matching on eyre error messages is
brittle; the 404 signal should stay in the type system.

**Implementation:** change `PdClient::send` so that on a non-success status it
returns a typed error `ApiError { status: StatusCode, body: String }` wrapped
via `eyre`. `try_get` uses `eyre::Report::downcast_ref::<ApiError>()` to
check the status. All other callers of `get`/`post`/`put`/`delete` are
unaffected because eyre still carries the formatted message.

```rust
// src/client.rs
#[derive(Debug, thiserror::Error)]
#[error("{formatted}")]
pub struct ApiError {
    pub status: StatusCode,
    pub body: String,
    pub formatted: String,
}

// ... in send(), replace `bail!("{}", format_api_error(status, &error_body))` with:
return Err(ApiError {
    status,
    body: error_body.clone(),
    formatted: format_api_error(status, &error_body),
}.into());

pub async fn try_get(&self, path: &str) -> Result<Option<Value>> {
    match self.send(Method::GET, path, None).await {
        Ok(v) => Ok(Some(v)),
        Err(e) => match e.downcast_ref::<ApiError>() {
            Some(api_err) if api_err.status == StatusCode::NOT_FOUND => Ok(None),
            _ => Err(e),
        },
    }
}
```

Adding `thiserror` is a single new dep (already common in the Rust ecosystem;
not in this crate yet but trivial to add). Alternative: hand-roll the error
type with `Display + std::error::Error` manually; equivalent but noisier.

Caller (handles the multi-match case explicitly):

```rust
// src/resources/incident/types.rs
async fn get(client: &PdClient, config: &Config, id_or_name: &str) -> Result<()> {
    if let Some(v) = client.try_get(&format!("/incidents/types/{}", id_or_name)).await? {
        print_value(&v, &config.output_format);
        return Ok(());
    }
    // Fallback: scan list for display-name match (case-insensitive).
    let all = client.get_all("/incidents/types", "incident_types").await?;
    let matches: Vec<&Value> = all
        .iter()
        .filter(|t| {
            t.get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .eq_ignore_ascii_case(id_or_name)
        })
        .collect();
    match matches.as_slice() {
        [] => eyre::bail!(
            "Incident type {:?} not found (tried ID, slug, and display name)",
            id_or_name
        ),
        [single] => {
            print_value(&serde_json::json!({ "incident_type": single }), &config.output_format);
            Ok(())
        }
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|t| t.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Display name {:?} matches {} incident types: {}. Use the slug or ID.",
                id_or_name, ids.len(), ids.join(", ")
            )
        }
    }
}
```

Regression test: two mock responses (one 404 for `/incidents/types/Managed%20Incident`,
one 200 for `/incidents/types?limit=25&offset=0` with a type whose `display_name`
is `Managed Incident`); assert the command prints that type.

### Fix 3: Real table rendering for list commands

Implement `print_value` so that when `format == Table` and the value is one of
the known list-response shapes, it renders a 2-D table. Fall back to JSON for
anything else. Use `colored` for subtle alignment and no external table
dependency; a small hand-rolled renderer is enough for 5 known shapes.

Known list shapes (one renderer per):

| Endpoint                                      | Columns                                           |
|-----------------------------------------------|---------------------------------------------------|
| `/priorities` (priority list)                 | name, description                                 |
| `/incidents/types` (incident-type list)       | id, name, display_name, enabled, parent_id        |
| `/incident_workflows` (incident-workflow list)| id, name, is_enabled                              |
| `/incident_workflows/triggers` (trigger list) | id, trigger_type, workflow_name, incident_types   |
| `/incident_workflows/actions` (action list)   | id, function_name, description (truncated to 80)  |

Dispatch in `output::print_value` by inspecting the top-level key
(`priorities`, `incident_types`, `incident_workflows`, `triggers`, `actions`).
Anything else falls back to JSON. `--output auto` continues to pick JSON when
stdout is piped; when it's a TTY, it picks table.

**Terminal width handling:** add the `terminal_size` crate (tiny, widely used
dep). If stdout is a TTY, get width via `terminal_size()`; otherwise assume
120. Pad columns to natural width up to the available total; if a row would
exceed the terminal, truncate the last column with `…`. Tests use a fixed
width of 120 via a test hook so output is deterministic.

**Null values:** render as empty string. `parent_id` for top-level incident
types is null; the column stays blank on those rows.

Regression test per shape: call the renderer with a known-good JSON and a
fixed width, capture the string, assert the header line and at least one data
line render as expected.

### Fix 4: Real sample config

Rewrite `pagerduty-cli.yml` to show only the fields the tool reads:

```yaml
# Sample configuration for pd (pagerduty-cli).
# Copy to ~/.config/pagerduty-cli/pagerduty-cli.yml and edit.
# CLI flags and env vars override anything set here.

# PagerDuty API token. Prefer PAGERDUTY_API_TOKEN env var in shells;
# only set it here when the env var is awkward (e.g. cron jobs).
# api-token: <paste-token-here>

# Subdomain for building HTML URLs to the PD web UI.
subdomain: tatari

# Default output format: auto | json | table.
# "auto" = table when stdout is a TTY, json when piped.
output-format: auto

# Log level: error | warn | info | debug | trace.
log-level: warn
```

No code change required; this is a docs/fixture fix.

### Fix 5: Workflow export falls back to trigger scan

The workflow object returned by `/incident_workflows/{id}?include[]=steps&include[]=triggers`
for PL7CP1H and PTFBRY3 has `steps: []` and `triggers: []`, even though the
account has workflows with the same name that *do* contain steps and triggers.
These "shadow" workflows live under different IDs (PHYROPU, PQ6QQID) that
aren't listed by `/incident_workflows`.

Change `export` to:

1. Fetch `/incident_workflows/{id}?include[]=steps&include[]=triggers` (as today).
2. If `triggers` is non-empty, proceed as today. Done.
3. If `triggers` is empty, call `/incident_workflows/triggers?limit=25`
   (via `get_all_no_offset` from Fix 1) and scan for any trigger whose
   `workflow.name` matches the stub workflow's `name`. If exactly one match,
   use it and fetch the full workflow at `workflow.id` to get real steps.
4. If zero matches: the workflow is genuinely empty; export with `trigger: null`
   (current behavior, now correctly communicated).
5. If multiple matches: bail with a clear error listing the candidate IDs and
   suggesting the user pass `--real-id <id>` (a new optional flag on `export`).

```rust
// src/resources/incident/workflows.rs
enum ShadowMatch { None, One(String), Many(Vec<String>) }

async fn find_shadow_workflow(client: &PdClient, name: &str) -> Result<ShadowMatch> {
    let triggers = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await?;
    let mut matched: Vec<String> = triggers
        .iter()
        .filter(|t| {
            t.get("workflow")
                .and_then(|w| w.get("name"))
                .and_then(|n| n.as_str())
                == Some(name)
        })
        .filter_map(|t| {
            t.get("workflow")
                .and_then(|w| w.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    matched.sort();
    matched.dedup();
    match matched.len() {
        0 => Ok(ShadowMatch::None),
        1 => Ok(ShadowMatch::One(matched.into_iter().next().expect("len==1"))),
        _ => Ok(ShadowMatch::Many(matched)),
    }
}

async fn export(client: &PdClient, id: &str, real_id: Option<&str>) -> Result<()> {
    let effective_id = real_id.unwrap_or(id);
    let resp = client
        .get(&format!(
            "/incident_workflows/{}?include[]=steps&include[]=triggers",
            effective_id
        ))
        .await?;
    let mut wf_raw = resp
        .get("incident_workflow")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Unexpected response: missing incident_workflow key"))?;

    let triggers_empty = wf_raw
        .get("triggers")
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    let steps_empty = wf_raw
        .get("steps")
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);

    // Shadow-workflow fallback: only when the user didn't force --real-id,
    // and both steps and triggers on the listed workflow are empty.
    if real_id.is_none() && triggers_empty && steps_empty {
        let name = wf_raw
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        match find_shadow_workflow(client, &name).await? {
            ShadowMatch::One(shadow_id) => {
                eprintln!(
                    "# Note: {} has no steps/triggers; exporting shadow workflow {} matched by name {:?}.",
                    id, shadow_id, name
                );
                let shadow_resp = client
                    .get(&format!(
                        "/incident_workflows/{}?include[]=steps&include[]=triggers",
                        shadow_id
                    ))
                    .await?;
                wf_raw = shadow_resp
                    .get("incident_workflow")
                    .cloned()
                    .ok_or_else(|| eyre::eyre!("Shadow response missing incident_workflow key"))?;
            }
            ShadowMatch::Many(ids) => bail!(
                "Workflow {} has no steps/triggers directly, and {} shadow workflows also match name {:?}: {}. \
                 Re-run with --real-id <id> to pick one.",
                id, ids.len(), name, ids.join(", ")
            ),
            ShadowMatch::None => {
                // Export as-is. trigger will be null; steps will be []. This correctly
                // reflects the account state (workflow genuinely empty).
            }
        }
    }

    // Existing logic: parse steps from wf_raw, look up the first trigger by ID
    // from the embedded triggers array, fetch its full record, serialize to YAML.
    let wf: IncidentWorkflow = serde_json::from_value(wf_raw.clone())
        .context("Failed to parse workflow")?;
    let trigger_yaml = if let Some(trigger_id) = wf_raw
        .get("triggers")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|t| t.get("id").and_then(|id| id.as_str()))
    {
        let t_resp = client
            .get(&format!("/incident_workflows/triggers/{}", trigger_id))
            .await?;
        let triggers_envelope = serde_json::json!({
            "triggers": [t_resp.get("trigger").cloned().unwrap_or(serde_json::Value::Null)]
        });
        find_trigger_for_workflow(&triggers_envelope,
            wf_raw.get("id").and_then(|v| v.as_str()).unwrap_or(""))
    } else {
        None
    };

    let def = api_to_definition(&wf, trigger_yaml);
    let yaml = serde_yaml::to_string(&def).context("Failed to serialize to YAML")?;
    println!("{}", yaml);
    Ok(())
}
```

Add `--real-id` to the `Export` clap variant. Document the fallback behavior
in the command's `--help`.

Regression test: mock the three scenarios (zero matches, one match, multiple
matches) with wiremock. Assert the one-match case prints the real steps; the
multi-match case bails with the expected error; the zero-match case prints
`trigger: null`.

### Fix 6: README auth section

Update the README's auth/setup section to reference `PAGERDUTY_API_TOKEN` (not
`PAGERDUTY_API_KEY`) and mention the secrets-repo file is
`pagerduty-api-token.age`. Single-file docs edit.

### Implementation Plan

Phases are ordered so that later phases can depend on earlier ones. Each phase
ends with `otto ci` passing. Each fix lands with its regression test; no
fix-without-test commits.

#### Phase 1: Pagination fix for trigger/action list
**Model:** sonnet
- Add `PdClient::get_all_no_offset` (body: single-page GET with `?limit=N` and
  no `offset`; see Fix 1 code block)
- Switch `src/resources/trigger.rs::list` from `get_all` to `get_all_no_offset`
- Switch `src/resources/action.rs::list` from `get_all` to `get_all_no_offset`
- Add `tests/trigger_list.rs`: wiremock server returns 400 for
  `/incident_workflows/triggers?limit=25&offset=0` and 200 for
  `/incident_workflows/triggers?limit=25`. Use `PdClient::with_base_url` to
  point at the mock. Assert the command succeeds.
- Mirror test for `action list`.

#### Phase 2: Typed ApiError + display-name lookup
**Model:** sonnet
- Add `thiserror` to `[dependencies]` via `cargo add thiserror`
- Introduce `ApiError` type in `src/client.rs`; replace the `bail!` in `send`
  with `ApiError { ... }.into()`. Verify existing error-formatting tests still
  pass (the `Display` output must match).
- Add `PdClient::try_get` (body per Fix 2 code block)
- Rewrite `src/resources/incident/types.rs::get` to call `try_get` and fall
  back to display-name scan on `Ok(None)`
- Add `tests/incident_type_get.rs`: mock 404 on
  `/incidents/types/Managed%20Incident`, mock 200 on the list endpoint with a
  type whose `display_name == "Managed Incident"`; assert the command prints it

#### Phase 3: Real sample config file
**Model:** sonnet
- Rewrite `pagerduty-cli.yml` per Fix 4
- Add a unit test in `src/config.rs` that loads `CARGO_MANIFEST_DIR/pagerduty-cli.yml`
  via `Config::load` with a `--config` pointing at that file (set
  `PAGERDUTY_API_TOKEN` in the test to allow `Config::load` to succeed even
  though the sample leaves `api-token` commented out)
- Verify `subdomain == "tatari"`, `log_level == "warn"`, `output_format` parses
  to `OutputFormat::Auto`

#### Phase 4: Workflow export shadow-ID fallback
**Model:** opus
- Depends on Phase 1 (`get_all_no_offset` must exist)
- Change `IncidentWorkflowAction::Export` clap variant from `{ id: String }`
  to `{ id: String, #[arg(long = "real-id")] real_id: Option<String> }`
- Update the handler dispatch in `workflows.rs::handle` to pass `real_id`
- Implement `find_shadow_workflow` and the new `export` per Fix 5
- Update `--help` text on the Export variant to explain the fallback and
  `--real-id` escape hatch
- Add wiremock tests for the three shadow-match cases (zero/one/multi)
- The existing `test_find_trigger_for_workflow_*` unit tests should still pass
  unchanged

#### Phase 5: Table rendering
**Model:** opus
- Rewrite `src/output.rs::print_value`: when `format == Table` (or `Auto` on a
  TTY), dispatch on the top-level key to one of five shape-specific renderers;
  otherwise render JSON
- Each renderer: print a header row, then one row per item. Pad columns to the
  widest value in that column. Null values render as empty string. Long values
  (e.g. action `description`) truncate to `min(80, terminal_width / cols)` with
  a trailing `…`
- Renderers live in a new `src/output/table.rs` submodule; `output.rs` stays
  the public entry
- Unit tests per renderer: construct the envelope JSON, call the renderer with
  a captured `impl Write`, assert the string contains the expected header and
  one representative row
- Verify `--output auto` on a piped call still prints JSON (existing behavior)

#### Phase 6: Release workflow (GitHub Actions)
**Model:** sonnet
- Add `.github/workflows/release.yml` triggered on tag pushes matching `v*`
- Build four targets via `cross` or matrix job:
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`
- Upload each binary as `pd-<target>` to the release
- Use `softprops/action-gh-release@v2` (pinned) or `gh release create` with
  the auto-provided `GITHUB_TOKEN`
- Test by pushing a throwaway `v0.1.99-test` tag first if needed (to a
  throwaway branch so it doesn't land on main)

#### Phase 7: README + shakedown + release
**Model:** sonnet
- Update README auth section: env var is `PAGERDUTY_API_TOKEN`; secrets file
  is `pagerduty-api-token.age`
- Run `otto ci`
- Re-run the shakedown against the live API (same commands as
  `docs/shakedown-v0.1.0.md`); confirm all six defects are resolved
- Append a "Resolved in v0.2.0" table to `docs/shakedown-v0.1.0.md`
- `bump -m` to `v0.2.0`; `git push && git push --tags` (triggers Phase 6 workflow)
- `cargo install --path .` to install locally
- Verify the GitHub release has all four binary assets
- Download the `x86_64-unknown-linux-gnu` asset to `/tmp/`, run `--version`,
  confirm it matches the locally installed binary

## Alternatives Considered

### Alternative 1: Rewrite `get_all` to handle endpoint-specific pagination quirks

- **Description:** Make `get_all` detect 400 responses on the offset request,
  retry without offset, cache the "no-offset" decision per endpoint.
- **Pros:** Single code path for all pagination.
- **Cons:** Hides the quirk; subtle retry logic; makes debugging harder;
  pushes complexity into shared infrastructure for a two-endpoint problem.
- **Why not chosen:** Two endpoints, known at design time. A sibling method is
  simpler and makes the quirk visible at the call site.

### Alternative 2: Display-name resolution in `get_all`-based scan upfront

- **Description:** Always scan the full types list in `incident-type get`,
  matching on ID, slug, or display name.
- **Pros:** Single code path; guaranteed to resolve.
- **Cons:** Two API calls instead of one for the common case (pass by slug/ID).
- **Why not chosen:** The try-direct-then-fallback pattern pays the extra call
  only when needed.

### Alternative 3: Use a crate like `comfy-table` or `tabled`

- **Description:** Pull in a table rendering crate.
- **Pros:** Rich formatting (alignment, wrapping, borders).
- **Cons:** New dependency; generic renderers don't know how to pick which
  columns matter for each PD resource.
- **Why not chosen:** Five well-known shapes; a tiny hand-rolled renderer is
  clearer and keeps the dep surface small. Can revisit if more shapes appear.

### Alternative 4: Leave `--output table` as JSON-fallback with a clearer doc

- **Description:** Remove the misleading `table` value from the enum or warn
  when the user asks for it.
- **Pros:** No implementation work.
- **Cons:** Every list command is harder to read in a terminal today.
- **Why not chosen:** The point of a CLI is human-readable output by default.
  The no-op is the real defect.

### Alternative 5: Fix the workflow shadow-ID issue via PagerDuty support

- **Description:** File a ticket with PagerDuty; don't work around it.
- **Pros:** Addresses root cause; helps other PD customers.
- **Cons:** No control over timing; CLI is broken in the meantime.
- **Why not chosen:** Do both. Work around in code now; file the ticket
  separately. Remove the workaround if/when PD fixes it.

## Technical Considerations

### Dependencies

Two new runtime dependencies, both small and widely used:

- `thiserror` - for the `ApiError` type in Phase 2. Adds ~0 to compile time
  (proc-macro only). Alternative: hand-roll `Display + Error` impls.
- `terminal_size` - for TTY width detection in Phase 5. Zero transitive deps
  beyond `libc`/`windows-sys`. Alternative: assume a fixed 120 cols, skip the
  crate entirely.

No new dev dependencies; `wiremock` and `assert_cmd` are already present.

### Performance

- Display-name fallback adds one extra GET on the 404 path; negligible.
- Shadow-workflow fallback adds one GET on empty-triggers export; negligible.
- Table rendering is local CPU; negligible vs network time.

### Security

- No new surface area. Sample config documents that `api-token` in the YAML
  file is a plaintext secret; add a comment recommending the env var instead.

### Testing Strategy

- **Regression tests per fix**, using `wiremock` where the PagerDuty API is
  involved. Each test encodes the shakedown's actual failure mode, so a
  future regression would fail the test before it fails the user.
- **Re-run the shakedown** as the final step before release (Phase 7).
  Update `docs/shakedown-v0.1.0.md` with "Resolved in v0.2.0" markers so the
  report stays the canonical record of what was broken and when it was fixed.

### Rollout Plan

Single release `v0.2.0`. No feature flags; every fix is a strict improvement.
Users upgrade with `cargo install --path .` from the repo, or by downloading a
prebuilt binary from the GitHub release (Phase 6 adds those artifacts).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Shadow-workflow fallback matches the wrong workflow (two with same name) | Low | Medium | Detect multi-match and bail with `--real-id` guidance. |
| Table renderer doesn't handle very long values (e.g., long descriptions) | Medium | Low | Truncate at column boundary with ellipsis. JSON remains for the full view. |
| `--output auto` table rendering breaks piped scripts that already grep the JSON output | Low | Medium | `auto` already emits JSON when stdout is piped; behavior unchanged for piped callers. |
| Shakedown re-run discovers new issues | Medium | Low | Scope creep into the v0.2.0 release is fine since this is a cleanup release; triage anything big into v0.3.0. |

## Open Questions

- [ ] File a PagerDuty support ticket for the stub-vs-implementation workflow
      ID split? Proposed: yes, after `v0.2.0` ships. Tracks whether we can
      remove the shadow fallback in a future release.
- [ ] Do we want table rendering for single-resource GETs (e.g. `incident-type
      get <id>`)? Out of scope for v0.2.0; revisit if asked.
- [ ] Should `get_all_no_offset` live on `PdClient` forever, or be a
      private helper inside `trigger.rs`/`action.rs`? Proposed: keep it on the
      client so tests can exercise it directly, and so future endpoints with
      the same quirk can reuse it.

## References

- `docs/shakedown-v0.1.0.md` - full shakedown report this fixes
- `docs/design/2026-04-15-pd-cli.md` - original design doc
- `src/client.rs` - pagination (`get_all`)
- `src/resources/trigger.rs`, `src/resources/action.rs` - failing list commands
- `src/resources/incident/types.rs` - display-name lookup site
- `src/output.rs` - table rendering placeholder
- `src/resources/incident/workflows.rs::export` - shadow-ID fallback site
- `pagerduty-cli.yml` - sample config
- PagerDuty API docs: https://developer.pagerduty.com/api-reference/
