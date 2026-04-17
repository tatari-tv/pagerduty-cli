# CLI Shakedown Report: pd v0.6.4

## Summary

| Metric | Count |
|--------|-------|
| Commands discovered | 68 (16 top-level + 52 subcommands) |
| Commands tested | 58 |
| Commands passed | 52 |
| Commands failed | 0 |
| Commands skipped (mutating) | 10 (create/update/delete/add/remove/bind/unbind/import) |
| Pipelines tested | 8 |
| Edge cases tested | 10 |
| Bugs found | 3 |
| Gaps found | 2 |

---

## Command Tree

```
pd
  rest                           GET/POST/PUT/DELETE passthrough
  incident
    list / get / create / update
    note list / add
    alert list / get
    trigger list / get / create / update / delete / bind / unbind
    type list / get / create / update / field list / create
    workflow list / get / create / update / delete / enable / disable / export / import
  priority list / verify
  action list / get
  user list / get
  team list / get / create / update / delete / member list / add / remove
  schedule list / get / create / update / delete / override list / create / delete
  escalation list / get / create / update / delete
  service list / get / create / update / delete / integration list / get / create / delete
  oncall list
  maintenance list / get / create / update / delete
  alert-grouping list / get / create / update / delete
  orchestration list / get / router get / update
  log list / get
  change list / get / create
  cache clear
```

---

## Command Results

### Global flags
| Flag | Result |
|------|--------|
| `--version` | `pd v0.6.4` - PASS |
| `--help` | Full help with Requires/Logs footer - PASS |
| `--output json` | Explicit JSON on all commands - PASS |
| `--output table` | Table on supported commands, silent JSON fallback on others - PARTIAL |
| `--no-cache` | Bypasses cache, hits API directly - PASS |
| `--api-token` | Override env token - not tested (no alternate token available) |

### Read-only list commands
| Command | Exit | Notes |
|---------|------|-------|
| `priority list` | 0 | Returns P1-P5, full JSON |
| `priority verify` | 0 | `✓ P1 ✓ P2 ✓ P3 ✓ P4` clean |
| `team list` | 0 | 21 teams returned |
| `user list` | 0 | 17 users returned |
| `service list` | 0 | 63 services returned |
| `escalation list` | 0 | Full list |
| `schedule list` | 0 | 19 schedules, all Terraform-managed |
| `oncall list` | 0 | All level-2 fallbacks showing; level-1 visible with pattern filter |
| `incident list` | 0 | Default: triggered+acknowledged last 1 day (empty during test) |
| `incident list --status resolved --since 2026-04-01` | 0 | 24 incidents |
| `action list` | 0 | Large AWS/Datadog/Slack/etc. action catalog |
| `change list` | 0 | Empty (no active change events) |
| `maintenance list` | 0 | Empty (no active windows) |
| `alert-grouping list` | 0 | 1 result: TVP intelligent grouping |
| `orchestration list` | 0 | 2 orchestrations (airflow-test, alertmanager-test) |
| `log list` | 0 | Recent log entries returned |
| `incident type list` | 0 | 4 types (Base, Major, Security, Managed) |
| `incident workflow list` | 0 | 2 workflows, both disabled |
| `incident trigger list` | 0 | 2 triggers (conditional + manual) |

### Get-by-name/ID commands (chained from list output)
| Command | Exit | Notes |
|---------|------|-------|
| `team get "SRE"` | 0 | Exact match |
| `team get "Data Platform"` | 0 | Exact match |
| `team get "Nonexistent Team XYZ"` | 1 | `Team "..." not found` - PASS |
| `service get "SRE P1"` | 0 | Tiered name match |
| `service get "nonexistent-service-xyz"` | 1 | `Service "..." not found` - PASS |
| `schedule get "SRE Schedule"` | 0 | Tiered name match |
| `escalation get "SRE P1 Escalation Policy"` | 0 | |
| `orchestration get "airflow-test"` | 0 | |
| `orchestration router get "airflow-test"` | 0 | Returns route rules |
| `incident type get "Base Incident"` | 0 | Display name match |
| `incident type get "managed_incident"` | 0 | Slug match |
| `incident workflow get "PTFBRY3"` | 0 | Note on stderr about shadow workflow |
| `incident workflow export "PTFBRY3"` | 0 | Exports shadow PQ6QQID, valid YAML |
| `incident get "NONEXISTENT_ID_XYZ"` | 1 | `API error 404 Not Found: Incident Not Found` |
| `incident get` (no arg) | 2 | Usage hint shown, no crash |
| `alert-grouping get "P3OHK8S"` | 0 | |
| `log get "R2YUI06L7Q8KAUC4Q2ITZ0E7ZG"` | 0 | |
| `action get "pagerduty.com:aws:invoke-aws-lambda:2"` | 0 | |
| `user get "scott.idler@tatari.tv"` | 0 | Email lookup works |
| `team member list "SRE"` | 0 | Returns members (see bug #2) |
| `team member list "Data Platform"` | 0 | |
| `incident note list Q3BBJ7H2A2861Y` | 0 | Empty list |
| `incident alert list Q3BBJ7H2A2861Y` | 0 | Returns full alert body |
| `incident type field list "PEK5BWB"` | 0 | Custom fields for Managed Incident type |
| `service integration list "SRE P1"` | 0 | 1 integration |

### Create --example flags (safe, read-only)
| Command | Exit | Notes |
|---------|------|-------|
| `incident create --example` | 0 | Clean commented YAML skeleton |
| `schedule create --example` | 0 | Clean YAML with rotation config |
| `escalation create --example` | 0 | Clean YAML with escalation rules |

### REST passthrough
| Command | Exit | Notes |
|---------|------|-------|
| `rest GET /incidents/types` | 0 | Returns incident types |
| `rest GET /priorities` | 0 | Returns priorities with pagination envelope |
| `rest GET /users/PZB4HAQ \| jq '{name,email,role}'` | 0 | Works cleanly |

---

## Output Format Matrix

| Command | `--output json` | `--output table` |
|---------|-----------------|------------------|
| `priority list` | PASS | PASS |
| `team list` | PASS | PASS (ID, NAME, DESCRIPTION) |
| `user list` | PASS | PASS (ID, NAME, EMAIL, ROLE) |
| `service list` | PASS | PASS (ID, NAME, ESCALATION_POLICY, STATUS) |
| `schedule list` | PASS | PASS (ID, NAME, TIME_ZONE, DESCRIPTION) |
| `escalation list` | PASS | PASS (ID, NAME, TEAMS, RULES) |
| `oncall list` | PASS | PASS (SCHEDULE, ESCALATION_POLICY, USER, LEVEL) |
| `team member list` | PASS | PASS (columns render) - EMAIL always blank (bug #2) |
| `service integration list` | PASS | PASS (ID, NAME, TYPE) |
| `incident type list` | PASS | PASS (ID, NAME, DISPLAY_NAME, ENABLED, PARENT_ID) |
| `incident workflow list` | PASS | PASS (ID, NAME, ENABLED) |
| `incident trigger list` | PASS | PASS (ID, TYPE, WORKFLOW, INCIDENT_TYPES) |
| `action list` | PASS | PASS (ID, FUNCTION_NAME, DESCRIPTION - with truncation) |
| `schedule override list` | PASS | PASS (requires --since) |
| `incident list` | PASS | FAIL - silently outputs JSON (no renderer) |
| `orchestration list` | PASS | FAIL - silently outputs JSON (no renderer) |
| `alert-grouping list` | PASS | FAIL - silently outputs JSON (no renderer) |
| `change list` | PASS | FAIL - silently outputs JSON (no renderer) |
| `log list` | PASS | FAIL - silently outputs JSON (no renderer) |
| `maintenance list` | PASS | FAIL - silently outputs JSON (no renderer) |

---

## Failures & Bugs

### Bug 1: Six resource types lack table renderers (cosmetic/feature gap)

**Commands affected:** `incident list`, `orchestration list`, `alert-grouping list`, `change list`, `log list`, `maintenance list`

`--output table` silently falls back to JSON for these because `output/table.rs::render()` has no match arm for the keys `incidents`, `orchestrations`, `alert_grouping_settings`, `change_events`, `log_entries`, or `maintenance_windows`. The fallback is documented in code but gives the user no warning.

**Severity:** Cosmetic/incomplete feature. JSON output is still valid and usable.

**Fix:** Add renderer arms in `table.rs` for each missing key.

---

### Bug 2: `team member list` EMAIL column always blank (cosmetic)

```
pd --output table team member list "SRE"
USER_ID  NAME             EMAIL  ROLE
P28VWE0  Keegan Ferrando         manager
```

`render_members` in `table.rs` calls `nested_str(r, "user", "email")`, but the API returns `members[]` with `user` as a reference object (`{id, summary, self, type}`) - not a full user record. There is no `email` field.

**Severity:** Cosmetic. The column always appears blank; user must use `--output json` to see emails.

**Fix:** Either drop the EMAIL column from the member table (it's unavailable without N+1 lookups), or replace with a note to use `pd user get <ID>` for full details.

---

### Bug 3: `schedule override list` requires `--since` but marks it optional (UX)

```
pd schedule override list "SRE Schedule"
Error: Command failed
Caused by: API error 400 Bad Request: Invalid Input Provided
Details: Since cannot be empty.
```

The `--since` and `--until` flags are shown as optional in `--help`, but the PagerDuty API requires `since` for override list requests. Without it, the CLI returns a 400 error with a confusing message.

**Severity:** UX bug. Easy to hit; error message doesn't tell you to add `--since`.

**Fix:** Default `--since` to some reasonable window (e.g., now - 7 days, now + 30 days) when not provided, OR mark `--since` as required, OR surface a better error: "Provide --since (e.g., --since 2026-01-01) to list overrides."

---

## Pipeline Recipes

All pipelines tested and verified working:

```bash
# Count total services
pd service list | jq '.services | length'
# Result: 63

# List all team names sorted
pd team list | jq -r '[.teams[].name] | sort[]'

# Filter services by name pattern
pd service list | jq -r '.services[] | select(.name | test("SRE")) | .name'
# Result: SRE P1 / SRE P2 / SRE P3

# Count resolved incidents in a date range
pd incident list --status resolved --since 2026-04-01 | jq '.incidents | length'
# Result: 24

# TSV of incident number, status, title
pd incident list --status resolved --since 2026-04-01 | \
  jq -r '.incidents[] | [.incident_number, .status, .title] | @tsv'

# Top services by incident volume
pd incident list --status resolved --since 2026-04-01 | \
  jq '[.incidents[] | .service.summary] | group_by(.) | map({service: .[0], count: length}) | sort_by(-.count)'
# Result: TVP=23, Direct Events P3=1

# Who is on-call for SRE right now
pd --output table oncall list "SRE"

# Get my user record
pd rest GET /users/PZB4HAQ | jq '{name: .user.name, email: .user.email, role: .user.role}'

# Look up user by email, then get their team memberships (two-step)
pd user get "scott.idler@tatari.tv" | jq '.user.id'
```

---

## Edge Cases

| Test | Expected | Actual | Result |
|------|----------|--------|--------|
| `pd incident get` (no arg) | Usage hint, exit 2 | `error: required argument <ID> not provided` | PASS |
| `pd incident get NONEXISTENT_ID_XYZ` | 404 error, exit 1 | `API error 404 Not Found: Incident Not Found` | PASS |
| `pd team get "Nonexistent Team XYZ"` | Not found, exit 1 | `Team "..." not found (tried ID and name)` | PASS |
| `pd service get "nonexistent-service-xyz"` | Not found, exit 1 | `Service "..." not found (tried ID and name)` | PASS |
| `pd schedule override list "SRE Schedule"` (no dates) | Clear error | 400 "Since cannot be empty" (bug #3) | FAIL |
| `pd incident list --status triggered --status acknowledged` | 0 open incidents | Empty list | PASS |
| `pd incident list --priority P1 --priority P2 --status resolved --since 2026-01-01` | Multi-flag filter | Empty (no P1/P2 incidents) | PASS |
| `pd oncall list --schedule "SRE Schedule"` | Error (no such flag) | "unexpected argument '--schedule'" | EXPECTED |
| `pd team list "Data"` | Pattern filter | Returns 4 Data* teams | PASS |
| `pd incident workflow get "PTFBRY3"` piped to `jq` | Valid JSON parse | Passes (note goes to stderr only) | PASS |

---

## Formatting Quality

**Table output observations:**
- Column alignment is clean and consistent
- Long values are truncated with `…` (e.g., action IDs truncated to terminal width)
- Truncation algorithm correctly shrinks the widest column first, not always the last
- Empty columns render as blanks (not "null") - correct
- Header row always present even for empty result sets (confirmed with `priorities: []`)
- Two-space separator between columns renders cleanly

**Minor observation:** `action list` truncates long IDs like `pagerduty.com:aws:auto-scaling-set-ins…` which makes copy-paste of IDs harder. The `action get <id>` command requires the full ID. Users who want to look up a specific action from the list output will need `--output json`.

---

## Observations

1. **Default output is `auto`** - JSON when piped (not a TTY), table when interactive. This is excellent for scripting.

2. **The `incident workflow get` shadow-workflow hint is correct** - When a workflow ID has a "shadow" (published copy), the note on stderr guides you to the right ID. Does not break JSON pipelines since it goes to stderr.

3. **Tiered name matching is consistent** - exact > starts-with > contains matching works the same way across team, service, schedule, escalation, and orchestration lookups.

4. **`priority verify` is Tatari-specific** - Hardcodes P1-P4 expectations. A nice operational shortcut but would fail in an org with different priority schemes.

5. **`oncall list` uses positional patterns, not `--schedule`/`--user` flags** - Less discoverable but works correctly. The help text says "match schedule, escalation policy, or user name" which is accurate.

6. **`incident list` has no `--service` filter** - Only `--team`, `--status`, `--priority`, `--since/--until`. To filter by service, users must pipe through `jq`.

7. **`user list --team`** - Handy filter for limiting to team members. Works correctly.

---

## Release Validation

| Check | Result |
|-------|--------|
| Git tag `v0.6.4` exists | YES |
| Tag type | Annotated (`tag` object) |
| GitHub release for `v0.6.4` | **NOT FOUND** |
| Latest GitHub release | `v0.2.1` (published 2026-04-16) |
| Versions without GitHub release | v0.3.0, v0.4.0, v0.5.0, v0.6.0, v0.6.1, v0.6.2, v0.6.3, v0.6.4 |
| v0.2.1 release assets | linux-amd64, linux-arm64, macos-arm64, macos-x86_64 + SHA256 checksums |
| Binary download tested | N/A (v0.6.4 release not published) |

**Finding:** The release pipeline (which correctly produced 4 platform binaries at v0.2.1) has not been run since v0.2.1. Any users installing from GitHub releases are running a binary that is 4 minor versions and many features behind the current state.

**Action needed:** Run the release pipeline for v0.6.4 (or tag and trigger the CI/CD workflow).
