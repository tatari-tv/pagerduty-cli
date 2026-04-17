# CLI Shakedown Report: pd v0.6.5

## Summary

| Metric | Count |
|--------|-------|
| Commands discovered | 72 |
| Commands tested | 47 |
| Commands passed | 47 |
| Commands skipped (mutating) | 25 |
| Pipelines tested | 3 |
| Edge cases tested | 4 |
| Bugs found | 5 |

---

## Command Results

### Global Options

```
pd --version       → pd v0.6.5   (exit 0) ✓
pd --help          → full tree   (exit 0) ✓
```

**Note:** `--output` is a **global flag only** — it must come before the subcommand:
```
pd --output table team list   ✓
pd team list --output table   ✗  ("unexpected argument '--output' found")
```

This is non-standard. Most CLIs accept flags after subcommands. The error message doesn't hint at the correct placement.

---

### Priority

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd priority list` | 0 | ✓ | ✓ |
| `pd priority verify` | 0 | ✓ (special) | — |

`pd priority verify` produces excellent output:
```
✓ P1
✓ P2
✓ P3
✓ P4

All expected priorities present
```

---

### Users

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd user list` | 0 | ✓ | ✓ |
| `pd user list "Keegan"` | 0 | ✓ | ✓ |
| `pd user list "NoSuchPerson"` | 0 | ✓ (empty) | ✓ |
| `pd user get P28VWE0` | 0 | ✗ raw JSON | ✓ |

`user list` table columns: ID, NAME, EMAIL, ROLE — well-chosen.

---

### Teams

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd team list` | 0 | ✓ | ✓ |
| `pd team get "SRE"` | 0 | ✗ raw JSON | ✓ |
| `pd team member list "SRE"` | 0 | ✓ (email missing — see bugs) | ✓ |

---

### Services

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd service list` | 0 | ✓ | ✓ |
| `pd service integration list "TVP"` | 0 | ✓ | ✓ |

`service list` columns: ID, NAME, ESCALATION_POLICY, STATUS — useful combination.

---

### Schedules

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd schedule list` | 0 | ✓ | ✓ |
| `pd schedule get "SRE Schedule"` | 0 | ✗ raw JSON | ✓ |

---

### Escalations

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd escalation list` | 0 | ✓ | ✓ |

`escalation list` columns: ID, NAME, TEAMS, RULES — useful, correctly shows rule count.

---

### On-Call

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd oncall list` | 0 | ✓ | ✓ |

`oncall list` columns: SCHEDULE, ESCALATION_POLICY, USER, LEVEL. SCHEDULE is empty for non-schedule-based entries (EP-level catchalls) — correct behavior.

---

### Incidents

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd incident list` | 0 | ✗ raw JSON | ✓ |
| `pd incident list --status resolved --since ...` | 0 | ✗ raw JSON | ✓ |
| `pd incident list --team "SRE"` | 0 | ✗ raw JSON | ✓ |
| `pd incident list --priority P1` | 0 | ✗ raw JSON | ✓ |
| `pd incident get Q3GN03S540DI18` | 0 | ✗ raw JSON | ✓ |
| `pd incident get 334` (by number) | 0 | ✗ raw JSON | ✓ ⭐ |
| `pd incident alert list <ID>` | 0 | ✗ raw JSON | ✓ |
| `pd incident note list <ID>` | 0 | ✗ raw JSON | ✓ |
| `pd incident type list` | 0 | ✓ | ✓ |
| `pd incident type get "Major Incident"` | 0 | ✗ raw JSON | ✓ |
| `pd incident workflow list` | 0 | ✓ | ✓ |
| `pd incident workflow export PL7CP1H` | 0 | ✓ YAML ⭐ | — |
| `pd incident workflow export PTFBRY3` | 0 | ✓ YAML ⭐ | — |

**Highlight:** `incident get` accepts both the PagerDuty ID (`Q3GN03S540DI18`) and the sequential incident number (`334`). Very ergonomic.

**Highlight:** `incident workflow export` has excellent shadow-workflow fallback logic. When a workflow ID has no steps (stub), it finds the real workflow by name automatically and notes it in a comment:
```
# Note: PL7CP1H has no steps/triggers; exporting shadow workflow PHYROPU matched by name "Custom Webhook Workflow".
```

---

### Actions

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd action list` | 0 | ✓ (ID truncated — see bugs) | ✓ |

---

### Orchestrations

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd orchestration list` | 0 | ✗ raw JSON | ✓ |
| `pd orchestration get "airflow-test"` | 0 | ✗ raw JSON | ✓ |
| `pd orchestration router get "airflow-test"` | 0 | ✗ raw JSON | ✓ |

---

### Logs & Changes

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd log list` | 0 | ✗ raw JSON (4.7 MB!) | ✓ |
| `pd change list` | 0 | ✗ raw JSON | ✓ |

---

### Alert Grouping

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd alert-grouping list` | 0 | ✗ raw JSON | ✓ |

---

### Maintenance

| Command | Exit | Table | JSON |
|---------|------|-------|------|
| `pd maintenance list` | 0 | ✗ raw JSON | ✓ |

---

### REST Passthrough

```bash
pd rest GET /incidents/types    → clean JSON, exit 0  ✓
```

---

### Cache

`pd cache clear` — skipped (mutating).

---

## Output Format Matrix

Commands where `--output table` works:

| Resource | list | get |
|----------|------|-----|
| priority | ✓ | — |
| user | ✓ | ✗ |
| team | ✓ | ✗ |
| team member | ✓ (empty email) | — |
| service | ✓ | ✗ |
| service integration | ✓ | ✗ |
| schedule | ✓ | ✗ |
| escalation | ✓ | ✗ |
| oncall | ✓ | — |
| **incident** | **✗** | **✗** |
| incident type | ✓ | ✗ |
| incident workflow | ✓ | — |
| action | ✓ (truncated) | ✗ |
| **orchestration** | **✗** | **✗** |
| **log** | **✗** | ✗ |
| **change** | **✗** | — |
| **alert-grouping** | **✗** | ✗ |
| **maintenance** | **✗** | ✗ |

Bold = `list` missing table rendering (gap vs. the commands that do have it).

---

## Bugs & Issues

### Bug 1 — `incident list` ignores `--output table` (severity: high)

**Command:** `pd --output table incident list`
**Expected:** table with columns like NUMBER, TITLE, STATUS, SERVICE, PRIORITY
**Actual:** raw API JSON

Incidents are the most-used resource in this tool. Having no table view for listing them makes the output unusable at a glance. Every other major `list` command has a table renderer.

---

### Bug 2 — `get` commands universally lack table rendering (severity: medium)

**Commands:** `incident get`, `user get`, `team get`, `schedule get`, `service get`, `orchestration get`, `incident type get`, `alert-grouping get`

All return the full raw PagerDuty API response envelope (with nested reference objects, URLs, `self` links, etc.) even with `--output table`. A `get` command is typically used to inspect a single resource — that's the highest-value place for a focused table or key-value summary layout.

Suggested columns per resource type:
- `incident get` → NUMBER, TITLE, STATUS, SERVICE, PRIORITY, CREATED_AT, RESOLVED_AT
- `user get` → ID, NAME, EMAIL, ROLE, TEAMS
- `team get` → ID, NAME, DESCRIPTION
- `schedule get` → ID, NAME, TIMEZONE, ESCALATION_POLICIES

---

### Bug 3 — `action list` truncates ID column (severity: low)

**Command:** `pd --output table action list`
**Example:** `pagerduty.com:aws:auto-scaling-set-ins…` — truncated with `…`

Action IDs are used as input to `action get` and workflow step definitions. Truncated IDs can't be copy-pasted. The table should either widen the column or wrap it.

---

### Bug 4 — `team member list` EMAIL column always empty (severity: low)

**Command:** `pd --output table team member list "SRE"`
**Output:**
```
USER_ID  NAME             EMAIL  ROLE
P28VWE0  Keegan Ferrando         manager
```

The PagerDuty teams membership API does not include email in the response. The column is rendered but always empty. Either enrich with a per-user lookup (expensive) or drop the EMAIL column from this specific table.

---

### Bug 5 — `pd log list` has no default date ceiling and returns enormous output (severity: medium)

**Command:** `pd log list`
**Result:** 4.7 MB of JSON with no feedback about response size.

`log list` should default to a recent time window (e.g., last 24 hours) matching the pattern from `incident list`. As-is, running it without `--since`/`--until` will overwhelm any terminal and is likely to hit API pagination limits silently.

---

## Pipeline Recipes

```bash
# Count incident types
pd --output json incident type list | jq '.incident_types | length'
# → 4

# List incident types as compact name/id/status
pd --output json incident type list | jq '[.incident_types[] | {id, name: .display_name, enabled}]'

# Get all team names sorted
pd --output json team list | jq -r '.teams[].name' | sort

# Find who is on-call for SRE right now
pd --output table oncall list | grep "SRE"

# Export all workflow definitions to YAML
for id in $(pd --output json incident workflow list | jq -r '.incident_workflows[].id'); do
  pd incident workflow export "$id" > /tmp/workflow-$id.yml
done
```

---

## Edge Cases

| Test | Exit | Result |
|------|------|--------|
| `pd incident get` (missing arg) | 2 | ✓ shows usage with required `<ID>` |
| `pd incident get INVALID_ID_DOES_NOT_EXIST` | 1 | ✓ "API error 404 Not Found: Incident Not Found" |
| `pd user list "DefinitelyNoSuchPersonExists12345"` | 0 | ✓ empty table with headers |
| `pd incident alert Q3GN03S540DI18` (forgot `list`) | 2 | ✓ "unrecognized subcommand" with usage hint |

Error messages use eyre formatting with location info (`src/client.rs:202:18`) — useful for debugging but may expose implementation details in production use.

---

## Release Validation

| Check | Result |
|-------|--------|
| Tag `v0.6.5` exists | ✓ |
| Tag type | annotated (`tag`) |
| GitHub release | ✓ published, not draft |
| Release author | `github-actions[bot]` |
| `linux-amd64` binary | ✓ present (`pd-v0.6.5-linux-amd64.tar.gz`, 5.2 MB) |
| `linux-arm64` binary | ✓ present |
| `macos-arm64` binary | ✓ present |
| `macos-x86_64` binary | ✓ present |
| SHA256 checksums | ✓ all 4 targets have `.sha256` sidecar |
| Checksum verification | ✓ `3847a557…` matches |
| Binary smoke test | ✓ `pd --version` → `pd v0.6.5` |
| Version match | ✓ identical to locally installed binary |

Missing: no Windows binary (`pd-v0.6.5-windows-x86_64.exe`). Not a concern if Windows is out of scope.

---

## Observations

1. **`--output` flag position** — Requiring `pd --output table <cmd>` instead of `pd <cmd> --output table` is the single biggest UX friction point. New users will try the latter and get a confusing error. Consider making `--output` also parseable after the subcommand, or at minimum improve the error message: "Did you mean `pd --output table ...`?".

2. **JSON envelope vs. table stripping** — `--output json` returns the full PagerDuty API envelope (with `limit`, `offset`, `more` pagination fields); `--output table` strips the envelope and renders just the items. This is a useful distinction but it is undocumented. Worth calling out in `--help`.

3. **Shadow workflow fallback** — The `incident workflow export` stub/shadow detection is genuinely clever and saves real operational pain. Worth documenting it more visibly (the `--real-id` flag to bypass it is buried).

4. **`pd priority verify`** — The Tatari-specific severity matrix check is a great example of domain-specific tooling. Works cleanly.

5. **`pd rest` passthrough** — Works correctly. Useful for one-off API exploration without leaving the tool.

6. **Incident number lookup** — `pd incident get 334` (by sequential number) working alongside `pd incident get Q3GN03S540DI18` (by ID) is a real ergonomic win during an incident when numbers are what's visible in alerts.
