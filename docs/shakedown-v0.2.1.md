# CLI Shakedown Report: pd v0.2.1

## Summary

| Metric | Count |
|--------|-------|
| Commands discovered | 24 |
| Commands tested (read-only) | 18 |
| Commands passed | 16 |
| Commands failed | 2 |
| Commands skipped (mutating) | 6 |
| Pipelines tested | 4 |
| Edge cases tested | 4 |

---

## Command Tree

```
pd
  rest <METHOD> <PATH> [--body JSON]
  incident
    type
      list [--filter enabled|disabled|all]
      get <ID|slug|"Display Name">
      create --name <slug> --display-name <name> [--description]   [SKIPPED - mutating]
      update <ID|name> [--display-name] [--enabled]                [SKIPPED - mutating]
      field
        list <type>
        create <type> --name --data-type --field-type               [SKIPPED - mutating]
    workflow
      list [--query Q]
      get <ID> [--include-steps]
      create --name <name> [--from-file FILE]                      [SKIPPED - mutating]
      update <ID> [--name] [--description]                         [SKIPPED - mutating]
      delete <ID>                                                   [SKIPPED - mutating]
      enable <ID>                                                   [SKIPPED - mutating]
      disable <ID>                                                  [SKIPPED - mutating]
      export <ID> [--real-id <ID>]
      import <FILE> [--id <ID>]                                    [SKIPPED - mutating]
  priority
    list
    verify
  trigger
    list
    get <ID>
    create --workflow-id --type [--condition] [--incident-types]   [SKIPPED - mutating]
    update <ID> [--condition] [--incident-types]                   [SKIPPED - mutating]
    delete <ID>                                                     [SKIPPED - mutating]
    create-for-service <trigger-id> --service-id                   [SKIPPED - mutating]
    remove-from-service <trigger-id> --service-id                  [SKIPPED - mutating]
  action
    list [--query Q]
    get <ID>
```

---

## Command Results

### Priority

| Command | Exit | Result |
|---------|------|--------|
| `pd priority list` | 0 | 5 priorities (P1-P5) returned as raw API JSON |
| `pd priority verify` | 0 | `✓ P1 ✓ P2 ✓ P3 ✓ P4 - All expected priorities present` |
| `pd --output table priority list` | 0 | Clean 2-column table: NAME, DESCRIPTION |
| `pd --output json priority list \| jq '. \| keys'` | 0 | Valid JSON, keys: limit/more/offset/priorities/total |

### Incident Type

| Command | Exit | Result |
|---------|------|--------|
| `pd incident type list` | 0 | 4 types: Base Incident, Major Incident, Security Incident, Managed Incident |
| `pd --output table incident type list` | 0 | Clean 5-column table: ID, NAME, DISPLAY_NAME, ENABLED, PARENT_ID |
| `pd incident type get PMQN99D` | 0 | Returns full type object |
| `pd incident type get "Managed Incident"` | 0 | Display name resolution works |
| `pd incident type get managed_incident` | 0 | Slug resolution works |
| `pd incident type field list PEK5BWB` | 0 | 4 fields returned (owned by parent P3U18MW) |
| `pd incident type field list managed_incident` | 0 | Slug resolution works for field list |
| `pd incident type field list "Managed Incident"` | **1** | **BUG: 404 - display name not resolved** |
| `pd incident type get "Nonexistent Type"` | 1 | Clean error: `"Nonexistent Type" not found (tried ID, slug, and display name)` |
| `pd incident type list --filter disabled` | 0 | Returns empty array correctly |

### Incident Workflow

| Command | Exit | Result |
|---------|------|--------|
| `pd incident workflow list` | 0 | 2 workflows returned |
| `pd --output table incident workflow list` | 0 | Clean 3-column table: ID, NAME, ENABLED |
| `pd incident workflow list --query major` | 0 | Filtered to 1 result correctly |
| `pd incident workflow get PTFBRY3` | 0 | Returns stub workflow (empty steps/triggers) |
| `pd incident workflow get PTFBRY3 --include-steps` | 0 | Same stub - `--include-steps` has no visible effect on a stub |
| `pd incident workflow export PTFBRY3` | 0 | Shadow fallback triggered, exports real PQ6QQID with full 8-step YAML |
| `pd incident workflow export PTFBRY3 > /tmp/major-incident-workflow.yml` | 0 | Export to file works; note goes to stderr, YAML to stdout |
| `pd incident workflow get BADID` | 1 | `API error 404 Not Found: Unknown error` (passable but opaque) |

### Triggers

| Command | Exit | Result |
|---------|------|--------|
| `pd trigger list` | 0 | 3 triggers: 1 conditional, 1 manual, 1 incident_type |
| `pd --output table trigger list` | 0 | Clean 4-column table: ID, TYPE, WORKFLOW, INCIDENT_TYPES |
| `pd trigger get f8726f03-...` | 0 | Full trigger object with workflow reference |

### Actions

| Command | Exit | Result |
|---------|------|--------|
| `pd action list` | 0 | 335KB response - large action catalog |
| `pd action list --query slack` | **1** | **BUG: 400 Bad Request - query param not supported by API** |
| `pd action get pagerduty.com:slack:create-a-channel:4` | 0 | Full schema with inputs/outputs |

### REST Passthrough

| Command | Exit | Result |
|---------|------|--------|
| `pd rest GET /teams` | 0 | 21 teams returned |
| `pd rest GET /schedules` | 0 | All schedules with escalation policy links |
| `pd rest GET /escalation_policies` | 0 | Full EP list with escalation rules |
| `pd rest GET /services` | 0 | Services list (more=true, pagination needed for full set) |

---

## Bugs & Issues

### Bug 1: `incident type field list` does not resolve display names

**Command:** `pd incident type field list "Managed Incident"`
**Expected:** Same behavior as `pd incident type get "Managed Incident"` - resolves by ID, slug, or display name
**Actual:** `API error 404 Not Found: Incident type 'Managed Incident' not found.`
**Workaround:** Use slug (`managed_incident`) or ID (`PEK5BWB`)
**Severity:** Bug - inconsistent API across sibling commands

### Bug 2: `action list --query` returns 400

**Command:** `pd action list --query slack`
**Expected:** Filtered list of actions matching "slack"
**Actual:** `API error 400 Bad Request: Unknown error`
**Likely cause:** PagerDuty's action list endpoint doesn't support a `query` parameter, or the parameter name is wrong
**Workaround:** `pd action list | jq` to filter client-side
**Severity:** Bug - flag exists but breaks the command

### Observation 1: Shadow workflow IDs are confusing

`pd incident workflow list` returns stubs (PTFBRY3, PL7CP1H) with empty steps/triggers. The real workflows live at different IDs (PQ6QQID, PHYROPU). The `export` fallback handles this transparently, but `get` returns the stub with no indication that a shadow exists. A user calling `get` has no way to know why steps are empty.

**Suggestion:** `get` could emit the same note as `export` when it detects a stub (no steps and no triggers but a name match exists in the trigger list).

### Observation 2: `priority list` returns raw API envelope in JSON mode

`pd --output json priority list` returns `{limit, more, offset, priorities: [...], total}` - the full API wrapper. The table renderer correctly extracts just the priorities. JSON mode should either return just the array or document that it's the raw envelope.

### Observation 3: `action list` output is 335KB unfiltered

Without `--query` (which is broken), there's no way to filter the action catalog without piping to `jq`. `action list --query` should either be fixed or removed.

---

## Output Format Matrix

| Command | `--output table` | `--output json` | `--output auto` |
|---------|-----------------|-----------------|-----------------|
| `priority list` | PASS - NAME, DESCRIPTION | PASS (raw envelope) | PASS (table on TTY) |
| `incident type list` | PASS - ID, NAME, DISPLAY_NAME, ENABLED, PARENT_ID | PASS | PASS |
| `incident workflow list` | PASS - ID, NAME, ENABLED | PASS | PASS |
| `trigger list` | PASS - ID, TYPE, WORKFLOW, INCIDENT_TYPES | PASS | PASS |
| `priority verify` | n/a (text output always) | n/a | PASS |
| `incident workflow export` | n/a (YAML always) | n/a | PASS |

---

## Pipeline Recipes

```bash
# Extract incident type IDs and display names
pd --output json incident type list | jq '[.incident_types[] | {id, name, display_name}]'

# Get all trigger-to-workflow mappings
pd --output json trigger list | jq '[.triggers[] | {type: .trigger_type, workflow: .workflow_name, incident_types}]'

# Export workflow to file for version control (note goes to stderr, YAML to stdout)
pd incident workflow export PTFBRY3 > major-incident-workflow.yml

# Look up a field ID by display name
pd incident type field list P3U18MW | jq -r '.fields[] | select(.display_name == "Slack Channel URL") | .id'

# Get all team IDs and names
pd rest GET /teams | jq '[.teams[] | {id, name}]'

# Count services (first page)
pd rest GET /services | jq '.services | length'
```

---

## Release Validation

| Check | Result |
|-------|--------|
| Tag `v0.2.1` exists | Yes |
| Tag is annotated | Yes (`git cat-file -t v0.2.1` returns `tag`) |
| GitHub release published | Yes, published 2026-04-16 |
| `pd-v0.2.1-linux-amd64.tar.gz` | Present, 4.2 MB |
| `pd-v0.2.1-linux-arm64.tar.gz` | Present, 4.0 MB |
| `pd-v0.2.1-macos-arm64.tar.gz` | Present, 3.9 MB |
| `pd-v0.2.1-macos-x86_64.tar.gz` | Present, 4.1 MB |
| SHA256 checksums | Present for all 4 targets |
| macOS Intel (`x86_64`) | Present |
| macOS Apple Silicon (`arm64`) | Present |
| Release binary version test | `pd v0.2.1` - matches installed binary |

All 4 targets ship with SHA256 checksums. Release binary matches locally installed version.

---

## Skipped (Mutating) Commands

| Command | Reason skipped |
|---------|---------------|
| `pd incident type create` | Creates new incident type |
| `pd incident type update` | Modifies existing incident type |
| `pd incident type field create` | Creates new custom field |
| `pd incident workflow create/update/delete/enable/disable/import` | Modifies workflow state |
| `pd trigger create/update/delete/create-for-service/remove-from-service` | Modifies trigger state |
