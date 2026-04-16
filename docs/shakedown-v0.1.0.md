# CLI Shakedown Report: pd v0.1.0

> **Status:** All defects resolved in **v0.2.0**. See
> `docs/design/2026-04-16-shakedown-fixes.md` for the fix design, and the
> table below for a per-defect resolution summary.
>
> | Defect | Resolved in v0.2.0 by |
> |---|---|
> | `pd trigger list` 400 | `get_all_no_offset` in `src/client.rs`; trigger::list switched |
> | `pd action list` 400 | same helper; action::list switched |
> | `pd incident-type get "Managed Incident"` 404 | `try_get` + display-name fallback in `src/resources/incident/types.rs::get` |
> | `pd --output table` no-op | per-shape renderers in `src/output/table.rs` for 5 list endpoints |
> | `pagerduty-cli.yml` has fake fields | rewritten to document the four real fields |
> | `pd incident-workflow export` returns `trigger: null` | shadow-workflow fallback by name + trigger-list scan fallback |
> | No GitHub release | `.github/workflows/release.yml` now targets binary name `pd` |


## Summary

| Metric | Count |
|--------|-------|
| Commands discovered | 24 |
| Commands tested | 19 |
| Commands passed | 17 |
| Commands failed | 2 |
| Commands skipped | 5 (mutating/destructive) |
| Pipelines tested | 4 |
| Edge cases tested | 6 |

**Version tested:** `v0.1.0-4-g9ffd3e9` (4 commits ahead of v0.1.0 tag)

**Auth:** `PAGERDUTY_API_TOKEN` env var (also accepts `--api-token` flag or `api-token` in `~/.config/pagerduty-cli/pagerduty-cli.yml`)

---

## Command Tree

```
pd
  rest <METHOD> <PATH> [--body <BODY>]
  incident-type
    list [--filter enabled|disabled|all]
    get <ID_OR_NAME>
    create --name --display-name [--description]      (MUTATING)
    update <ID_OR_NAME> [--display-name] [--description] [--enabled]  (MUTATING)
    field
      list <TYPE_ID_OR_NAME>
      create --name --data-type --field-type <TYPE_ID_OR_NAME>  (MUTATING)
  priority
    list
    verify
  incident-workflow
    list [--query]
    get <ID> [--include-steps]
    create ...                  (MUTATING)
    update ...                  (MUTATING)
    delete <ID>                 (DESTRUCTIVE)
    enable <ID>                 (MUTATING)
    disable <ID>                (MUTATING)
    export <ID>
    import <FILE> [--id <ID>]   (MUTATING)
  trigger
    list
    get <ID>
    create --workflow-id --type [--condition] [--incident-types]  (MUTATING)
    update <ID> [--condition] [--incident-types]   (MUTATING)
    delete <ID>                 (DESTRUCTIVE)
    create-for-service --service-id <TRIGGER_ID>   (MUTATING)
    remove-from-service --service-id <TRIGGER_ID>  (MUTATING)
  action
    list [--query]
    get <ID>
```

---

## Command Results

### Global

| Command | Exit | Result |
|---------|------|--------|
| `pd --version` | 0 | `pd v0.1.0-4-g9ffd3e9` |
| `pd --help` | 0 | Full usage printed |
| `pd --api-token invalid-token incident-type list` | 1 | `API error 401 Unauthorized` - clean error |

### priority

| Command | Exit | Result |
|---------|------|--------|
| `pd priority list` | 0 | Returns P1-P5 with descriptions, colors, IDs |
| `pd priority verify` | 0 | `✓ P1 ✓ P2 ✓ P3 ✓ P4 / All expected priorities present` |

### incident-type

| Command | Exit | Result |
|---------|------|--------|
| `pd incident-type list` | 0 | 4 types: Base Incident, Major Incident, Security Incident, Managed Incident |
| `pd incident-type list --filter enabled` | 0 | All 4 (all are enabled) |
| `pd incident-type list --filter disabled` | 0 | Empty array |
| `pd incident-type list --filter invalid-value` | 2 | Clap error with valid values shown |
| `pd incident-type get P3U18MW` | 0 | Lookup by ID works |
| `pd incident-type get incident_default` | 0 | Lookup by slug works |
| `pd incident-type get managed_incident` | 0 | Works |
| `pd incident-type get "Managed Incident"` | 1 | **FAILS** - display name not supported, only slug/ID |
| `pd incident-type get nonexistent-id` | 1 | `API error 404: Incident type 'nonexistent-id' not found.` |
| `pd incident-type get` (no arg) | 2 | Clap error, usage shown |
| `pd incident-type field list incident_default` | 0 | 4 fields: Slack Channel URL, Slack Message TS, Jira Issue URL, Confluence Page URL |
| `pd incident-type field list managed_incident` | 0 | Returns parent type fields (inherited from incident_default) |

### incident-workflow

| Command | Exit | Result |
|---------|------|--------|
| `pd incident-workflow list` | 0 | 2 workflows: Custom Webhook Workflow, Major Incident Workflow |
| `pd incident-workflow list --query Major` | 0 | 1 result filtered correctly |
| `pd incident-workflow get PL7CP1H` | 0 | Returns workflow, `steps: []`, `triggers: []` |
| `pd incident-workflow get PTFBRY3 --include-steps` | 0 | Steps still `[]` - workflows have no steps populated |
| `pd incident-workflow get BADID000` | 1 | `API error 404 Not Found` |
| `pd incident-workflow export PL7CP1H` | 0 | YAML output, `steps: []`, `trigger: null` |
| `pd incident-workflow export PTFBRY3` | 0 | YAML output, `steps: []`, `trigger: null` |

### trigger

| Command | Exit | Result |
|---------|------|--------|
| `pd trigger list` | 1 | **BUG** - `API error 400 Bad Request` |
| `pd trigger get 52aff97f-...` | 0 | Returns trigger with workflow reference |

### action

| Command | Exit | Result |
|---------|------|--------|
| `pd action list` | 1 | **BUG** - `API error 400 Bad Request` |

### rest (passthrough)

| Command | Exit | Result |
|---------|------|--------|
| `pd rest GET /incidents/types` | 0 | Raw API response |
| `pd rest GET /incident_workflows/triggers` | 0 | 3 triggers returned |
| `pd rest GET /incident_workflows/actions` | 0 | ~35KB, many actions |
| `pd rest GET /services?limit=3` | 0 | Paginated services list |
| `pd rest GET /incident_workflows/actions?limit=25&offset=0` | 1 | Confirms 400 with offset |
| `pd rest GET /incident_workflows/actions?limit=25` | 0 | Works without offset |

---

## Output Format Matrix

| Command | --output json | --output table | --output auto |
|---------|--------------|----------------|---------------|
| incident-type list | JSON | JSON (fallback) | JSON (non-TTY) |
| priority list | JSON | JSON (fallback) | JSON (non-TTY) |
| incident-workflow list | JSON | JSON (fallback) | JSON (non-TTY) |

`--output table` is a placeholder - falls back to JSON output (documented "Phase 1" in `src/output.rs`). All three modes currently produce identical output in non-TTY context.

---

## Failures & Bugs

### Bug 1: `pd trigger list` and `pd action list` return 400 (CRITICAL)

**Commands:** `pd trigger list`, `pd action list`

**What happened:** Both fail with `API error 400 Bad Request: Unknown error`

**Root cause:** `client::get_all()` always appends `?limit=25&offset=0` to paginate. The `/incident_workflows/triggers` and `/incident_workflows/actions` endpoints accept `limit` but reject `offset` - `?limit=25&offset=0` returns 400, `?limit=25` returns 200.

**Evidence:**
```
pd rest GET "/incident_workflows/triggers?limit=25&offset=0"  # 400
pd rest GET "/incident_workflows/triggers?limit=25"            # 200, 3 triggers
pd rest GET "/incident_workflows/actions?limit=25&offset=0"   # 400
pd rest GET "/incident_workflows/actions?limit=25"             # 200, many actions
```

**Fix:** These two commands should use `client.get()` directly instead of `client.get_all()`, or `get_all` needs a flag to suppress `offset`. Given neither endpoint appears to have more than fits in a single page, a simple `client.get()` call is the right fix.

Files: `src/resources/trigger.rs:46`, `src/resources/action.rs:21`

---

### Bug 2: `pd incident-type get` does not resolve by display name

**Command:** `pd incident-type get "Managed Incident"`

**What happened:** Returns 404 - the `get` command only does a direct API lookup by the ID or slug passed. "Managed Incident" is a display name, not a slug (`managed_incident`).

**Severity:** UX issue - the help text says `<ID_OR_NAME>` which implies display names should work. The `trigger.rs` already has `resolve_incident_type_ids` which resolves display names. The same logic should apply to `incident-type get`.

---

### Observation: Workflow ID mismatch between list and triggers

`pd incident-workflow list` returns IDs `PL7CP1H` / `PTFBRY3`, but the triggers reference `PHYROPU` / `PQ6QQID` for the same-named workflows. These are the "published" vs "draft" versions of workflows in PagerDuty's API. `pd incident-workflow export` therefore shows `trigger: null` because it looks up triggers by the published workflow ID, but all triggers point to the draft IDs.

This may be unavoidable given PagerDuty's data model, but it means `export` always shows no triggers, which is misleading.

---

## Pipeline Recipes

```bash
# List all incident type IDs
pd incident-type list | jq '[.incident_types[].id]'
# => ["P3U18MW", "PMQN99D", "PIDM8XQ", "PEK5BWB"]

# Extract id+name+display for all types
pd incident-type list | jq '.incident_types[] | {id, name, display_name}'

# Count incident types
pd incident-type list | jq '.incident_types | length'

# Export workflow to file
pd incident-workflow export PL7CP1H > workflow.yml

# Get a trigger using ID from rest passthrough
pd rest GET /incident_workflows/triggers | jq '.triggers[0].id'
# Then: pd trigger get <that-id>

# List all custom fields on the base type
pd incident-type field list incident_default | jq '.fields[] | {name, data_type, display_name}'
```

---

## Edge Cases

| Case | Command | Result |
|------|---------|--------|
| Missing required arg | `pd incident-type get` | Exit 2, clap usage error |
| Invalid filter value | `pd incident-type list --filter bad` | Exit 2, clap error with valid values |
| Nonexistent ID | `pd incident-type get nonexistent-id` | Exit 1, `404 Not Found` with detail |
| Bad workflow ID | `pd incident-workflow get BADID000` | Exit 1, `404 Not Found` |
| Invalid API token | `pd --api-token invalid incident-type list` | Exit 1, `401 Unauthorized` |
| Display name as ID | `pd incident-type get "Managed Incident"` | Exit 1, 404 - not supported |

---

## Release Validation

- **Tag `v0.1.0`:** exists locally, annotated
- **GitHub release:** not published - no releases exist on `tatari-tv/pagerduty-cli`
- **Installed binary:** `~/.cargo/bin/pd` v0.1.0-4-g9ffd3e9 (4 commits ahead of tag)

---

## Observations

1. **`PAGERDUTY_API_TOKEN` vs `PAGERDUTY_API_KEY`:** The secrets file was named `pagerduty-api-key.age` (now renamed to `pagerduty-api-token.age`). The env var name should be aligned across the secrets repo, env loading, and the CLI's expected var name. Now consistent.

2. **No `--output table` rendering:** All three output modes produce identical JSON. The `auto` mode is useful (JSON when piped, human-readable when TTY) but currently both modes produce the same thing. Table rendering would make `pd priority list` or `pd incident-type list` much more scannable in a terminal.

3. **Config file is a stub:** `pagerduty-cli.yml` contains sample fields (`name`, `age`, `debug`) that don't match the actual config struct. Should show real supported fields: `api-token`, `log-level`, `output`.

4. **`pd incident-workflow export` shows `trigger: null` for all workflows** due to the draft/published ID mismatch. Exporting a workflow that has live triggers should ideally show them.

5. **`pd rest` is very useful** for ad-hoc exploration. Works well as a debugging tool. The `/services`, `/incident_workflows/triggers`, and `/incident_workflows/actions` endpoints all work fine through it.

6. **`pd priority verify` is a nice touch** - human-readable confirmation that P1-P4 are configured correctly. Clean, actionable output.
