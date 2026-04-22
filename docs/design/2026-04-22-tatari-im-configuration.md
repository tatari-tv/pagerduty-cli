# Design Document: Tatari Incident Management Configuration

**Author:** Scott Idler
**Date:** 2026-04-22
**Status:** In Review
**Review Passes Completed:** 5/5

## Summary

This document covers the remaining work to complete Tatari's Incident Management (IM)
setup in PagerDuty. The `pd` CLI is built and tested. Incident roles and incident types
are configured. What remains is: one CLI gap to fix, Jira integration to verify, and
incident workflow YAML files to create and import for all three incident types (Managed,
Security, Business).

## Problem Statement

### Background

Tatari is migrating from OpsGenie to PagerDuty. The IM system defines three incident
types (Managed, Security, Business), each requiring a structured response workflow. The
`pd` CLI was built to replace the 970-line clickops guide with repeatable, version-controlled
commands.

The CLI is implemented and shaken down (v0.6.5, all bugs fixed). Roles and incident types
are now configured. Workflows are the last major piece.

### Completed Work

| Item | Status | Method |
|------|--------|--------|
| Priority levels P1-P5 | Done | Pre-existing |
| Slack integration | Done | Pre-existing |
| Incident type: Base | Done | Pre-existing |
| Incident type: Major | Done | Pre-existing |
| Incident type: Security | Done | Pre-existing |
| Incident type: Managed | Done | Pre-existing |
| Incident type: Business | Done | `pd rest POST` (CLI gap workaround) |
| Role: Incident Commander | Done (enabled) | Clickops (no REST API) |
| Role: Tech Lead | Done (created) | Clickops (no REST API) |
| Role: Comms Lead | Done (created) | Clickops (no REST API) |
| Role: Customer Liaison | Done (disabled) | Clickops (no REST API) |
| Jira Cloud integration | In Progress | Clickops (OAuth, browser-only) |

### Problem

Three items block a complete, fully automated IM setup:

1. **CLI gap:** `pd incident type create` has no `--parent` flag. Business Incident was
   created via `pd rest POST` as a workaround. The gap should be fixed so future incident
   types can be created without raw API calls.

2. **Workflows not configured:** WF1-WF5 (Managed Incident) exist as a clickops guide
   and a design doc, but have not been created in PD. Security Incident and Business
   Incident have no workflows at all.

3. **Jira integration unverified:** The Jira Cloud OAuth connection must be confirmed
   working before the Jira action in WF1 can be tested end-to-end.

### Goals

- Fix the `--parent` flag gap in `pd incident type create`
- Create workflow YAML definitions for Managed Incident (WF1-WF5), Security Incident, and Business Incident
- Import all workflows via `pd incident workflow import`
- Verify Jira Cloud integration and test WF1 Jira action
- Execute the WF1-WF5 test matrix and rollout sequence

### Non-Goals

- Terraform changes (teams, schedules, escalation policies are out of scope)
- Incident role automation (no REST API; roles are clickops-only by PD design)
- OAuth / Jira Cloud integration setup (browser-only flow; must be done manually)
- Event orchestration changes
- Jeli PIR integration (deferred; noted in clickops guide future work)
- Status update templates (deferred)

## Proposed Solution

### Overview

Three phases:

1. **Fix CLI gap** - add `--parent` to `pd incident type create`
2. **Author workflow YAMLs** - create YAML files for all three incident types, commit to `workflows/`
3. **Import and test** - use `pd incident workflow import` to push all workflows, run test matrix, roll out

### Architecture

No new architecture. The existing `pd` CLI (`pd incident workflow import`, `pd incident workflow export`,
`pd rest`) handles everything except the `--parent` gap.

Workflow YAML files live in `workflows/` at the repo root:

```
workflows/
  managed-incident-response.yml      # WF1
  incident-visibility.yml            # WF2 (base/major only - excludes managed/security/business)
  auto-manage-p1.yml                 # WF3
  auto-manage-p1-escalation.yml      # WF4a
  managed-priority-changed.yml       # WF4b
  security-incident-response.yml     # WF5 (new)
  business-incident-response.yml     # WF6 (new)
```

### Role Assignment per Incident Type

Three roles apply to every incident. The person filling each role varies by incident type:

| Role | Managed Incident | Security Incident | Business Incident |
|------|-----------------|-------------------|-------------------|
| IC (Incident Commander) | Engineering Manager | Engineering Manager or Head of Security | Engineering Manager |
| TL (Tech Lead) | Engineer (affected area) | Security Engineer | Engineer/Manager (business-knowledgeable) |
| CL (Comms Lead) | Engineer/Manager | Engineer/Manager | CSM or business-side contact |

This is policy, not PD configuration. It should be documented in Confluence INC.

### Incident Workflow Summary

#### Managed Incident (WF1-WF5) - unchanged from clickops guide

| WF | Name | Trigger | Condition | Action |
|----|------|---------|-----------|--------|
| WF1 | Managed Incident Response | Incident triggered | Type = Managed Incident | Full response: Slack channel, topic, status card, Jira INC, bookmarks, #incidents post, 15-min delay, status reminder |
| WF2 | Incident Visibility | Incident triggered | Type is not Managed AND Type is not Security AND Type is not Business | Post to #incidents (lightweight, for base/major incidents only) |
| WF3 | Auto-Manage P1 | Incident triggered | Priority = P1 | Set type to Managed Incident |
| WF4a | Auto-Manage on P1 Escalation | Priority changes | Priority = P1 AND Type != Managed | Set type to Managed Incident |
| WF4b | Priority Changed | Priority changes | Type = Managed Incident | Post cadence update to channel + #incidents, update topic |

#### Security Incident (new)

| WF | Name | Trigger | Condition | Action |
|----|------|---------|-----------|--------|
| WF5 | Security Incident Response | Incident triggered | Type = Security Incident | Create Slack channel, post status card (security template), create Jira INC ticket, post to #incidents, prompt for IC/TL/CL role assignment |

Security Incident response is similar in structure to Managed Incident but with a security-specific
status card template and potentially a narrower escalation path (Head of Security in the IC slot).

#### Business Incident (new)

| WF | Name | Trigger | Condition | Action |
|----|------|---------|-----------|--------|
| WF6 | Business Incident Response | Incident triggered | Type = Business Incident | Create Slack channel, post status card (business template including partner name, data type, backfill status), create Jira INC ticket, post to #incidents, prompt for IC/TL/CL role assignment |

Business Incident status card must expose fields relevant to the process: partner name, affected data
types, estimated resolution, backfill decision status. These are not standard PD incident fields - they
will be filled manually in the Slack channel by the Partner Liaison role.

### Data Model

No new data model. The existing YAML schema from the original design doc applies:

```yaml
workflow:
  name: <string>
  description: <string>
  is-enabled: false  # always false on import; enable manually after testing
  steps:
    - name: <string>
      action-id: <domain>.<package>.<function>
      inputs:
        - name: <input_name>
          value: <liquid_template_or_literal>

trigger:
  trigger-type: incident_type | conditional | manual
  incident-types:           # for incident_type trigger
    - <name>
  condition: "<pcl_string>" # for conditional trigger
```

### API Design Changes

#### Fix: `pd incident type create --parent`

Current:
```
pd incident type create --name <name> --display-name <display> [--description <desc>]
```

After fix:
```
pd incident type create --name <name> --display-name <display> [--description <desc>] [--parent <id-or-name>]
```

The `--parent` flag resolves the name to an ID using the existing name-to-ID cache, then passes
`"parent_type": "<id>"` in the POST body. PD requires `parent_type` as a plain string ID (not an
object reference).

### Implementation Plan

#### Phase 1: Fix `--parent` flag on incident type create
**Model:** sonnet

- Add `--parent <id-or-name>` optional flag to `pd incident type create` in `cli.rs`
- Resolve parent name to ID via `resolve_incident_type()` in `resources/incident/types.rs`
- Pass `parent_type: Option<String>` in the create request body
- Return clear error if parent name doesn't resolve (e.g., `error: incident type "Foo" not found`)
- Test error case: `pd incident type create --name test --display-name "Test" --parent "Nonexistent"`
- Test: `pd incident type create --name test_type --display-name "Test" --parent "Base Incident"`
- Clean up: delete the test type after confirming the flag works
- `otto ci`

#### Phase 2: Author workflow YAML files
**Model:** opus

- Create `workflows/` directory at repo root
- Author `managed-incident-response.yml` (WF1) - 8 steps, exact Liquid templates from clickops guide
- Author `incident-visibility.yml` (WF2) - 1 step
- Author `auto-manage-p1.yml` (WF3) - 1 step
- Author `auto-manage-p1-escalation.yml` (WF4a) - 1 step
- Author `managed-priority-changed.yml` (WF4b) - 3 steps
- Author `security-incident-response.yml` (WF5) - modeled on WF1 with security-specific template
- Author `business-incident-response.yml` (WF6) - modeled on WF1 with business-specific template
- Verify all YAML files are valid before import by attempting `pd incident workflow import <file>` against the test service; any parse error surfaces immediately before PD is touched
- Commit all YAML files to `workflows/`

Key inputs for authoring (gather these FIRST, before writing any YAML):
- **Action IDs:** `pd --output json action list > /tmp/pd-actions.json` - do not use training-data IDs
- **Liquid variables:** `pd incident workflow export <any-existing-id>` to see what template variables PD exposes
- **PCL syntax:** `pd rest GET /incident_workflows/triggers` to inspect existing trigger condition strings

#### Phase 3: Verify Jira integration
**Model:** sonnet (ops task, not code)

- Confirm Jira Cloud OAuth connection in PD: **Integrations** > **Jira Cloud**
- Verify `INC` project visible and `Incident (PD)` issue type exists
- If not connected: complete OAuth flow via browser (clickops, cannot be automated)
- Create a manual test incident, use **More Actions** > **Create Jira Issue** to verify end-to-end
- Clean up test issue

#### Phase 4: Import and test
**Model:** sonnet (ops task)

- Run `pd action list` and resolve all action IDs used in the YAML files; update files if needed
- Import each workflow disabled: `pd incident workflow import workflows/<file>.yml`
- Verify all 7 workflows appear in PD: `pd incident workflow list`
- Execute the test matrix from the clickops guide (Tests 1-6), adding:
  - Test 7: Security Incident Response (create incident with type = Security Incident, verify WF5)
  - Test 8: Business Incident Response (create incident with type = Business Incident, verify WF6)
- Document pass/fail for each test
- Fix any YAML issues, re-import

#### Phase 5: Rollout
**Model:** sonnet (ops task)

Enable workflows in order, monitoring between each:

| Step | Workflow | Wait |
|------|----------|------|
| 1 | WF2: Incident Visibility | 1 day |
| 2 | WF3: Auto-Manage P1 | 1 day |
| 3 | WF1: Managed Incident Response | 2-3 days |
| 4 | WF4a: Auto-Manage on P1 Escalation | 1 day |
| 5 | WF4b: Priority Changed | 1 day |
| 6 | WF5: Security Incident Response | 2-3 days |
| 7 | WF6: Business Incident Response | 2-3 days |
| 8 | Remove stale Slack `#inc-*` connections | after 1 week |

Enable command: `pd incident workflow enable <name-or-id>`

## Alternatives Considered

### Alternative 1: Keep Security and Business Incident as manual (no workflows)
- **Description:** Only build WF1-WF5 for Managed Incident; Security and Business rely on manual declaration
- **Pros:** Less work now; Security and Business incidents are less frequent
- **Cons:** Inconsistent experience - Managed Incident gets automation, others don't. Responders won't know what to do during a Security or Business incident without tooling to guide them
- **Why not chosen:** The whole point of IM setup is consistency. Partial automation creates confusion about which incident types are "real."

### Alternative 2: Single unified workflow with type-based branching
- **Description:** One workflow that branches on incident type using conditional logic
- **Pros:** Single YAML to maintain
- **Cons:** PD's Liquid template engine may not support branching. PCL conditions operate at the trigger level, not inside workflow steps. Separate workflows with separate triggers is the PD-native model
- **Why not chosen:** PD's architecture pushes toward separate workflows per type

## Technical Considerations

### Dependencies

- `pd incident workflow import` must be working (it is, per v0.6.5 shakedown)
- `pd action list` must return valid action IDs (it does, per shakedown)
- Jira Cloud integration must be connected before WF1/WF5/WF6 Jira steps can be tested
- Slack integration must be connected (it is, per clickops guide)

### Performance

Not a concern. 7 workflow imports + 7 trigger creates = 14 API calls total.

### Security

No new security considerations. Workflows execute with the account's PD permissions. Jira actions
use the OAuth token from the Jira Cloud integration - no credentials in workflow YAML.

### Testing Strategy

- **Unit:** None for the YAML files themselves; they're data, not code
- **Manual integration:** Full test matrix from clickops guide (Tests 1-8)
- **CLI fix:** `otto ci` after adding the `--parent` flag

### Rollout Plan

See Phase 5 above. Workflows built disabled, enabled in staged order with monitoring.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Action IDs in YAML don't match live PD action IDs | Medium | High | Run `pd action list` before authoring YAMLs; don't use training-data IDs |
| Liquid template variables not supported in some action inputs | Medium | Medium | Test WF1 step-by-step on test service; fall back to static text where templates fail |
| Jira Cloud integration is not connected | Medium | Medium | Test in Phase 3 before authoring WF1 Jira step; worst case, disable Jira action and add manually post-connection |
| WF3 + WF1 race condition: both fire on P1 | Low | Medium | WF3 sets type then WF1 fires on type change; PD triggers are event-driven so ordering should be correct. Monitor in test |
| Security/Business workflow templates wrong (no prior art) | Medium | Low | Export WF1 after import and compare structure; Security/Business are structurally identical, only the status card text differs |
| Stale Slack `#inc-*` connections cause duplicate posts | Low | Low | Do not remove until WF1/WF2 have been stable for 1 week |

## Open Questions

- [ ] What are the exact action IDs for Slack, Jira, and PD Incident Management actions? (resolve with `pd action list` in Phase 4)
- [ ] Does the Jira Cloud integration support Liquid template input in the Summary field? (test in Phase 3)
- [ ] Does PD's Liquid engine support the `| date:` filter in action inputs? (test with WF1 channel name action)
- [ ] Should WF5 (Security) page the Head of Security role automatically, or just prompt for IC assignment? (policy question for Scott/Andy)
- [ ] Should WF6 (Business) include a Jira INC ticket, or track via a separate Partner Incident Log? (the Business Incident spec says Jira `INC` project with `business-incident` label)
- [ ] Does PD's PCL support multi-clause `AND` with `is not` comparisons needed for WF2's expanded exclusion condition? (verify by inspecting existing trigger conditions or testing a simple two-clause `AND` first)

## References

- `docs/design/2026-04-15-pd-cli.md` - original CLI design doc (implemented)
- `docs/tatari/managed-incident-clickops-guide.md` - 970-line UI walkthrough (the source of WF1-WF5 action configs)
- `docs/tatari/managed-incident-workflow-config.md` - WF1-WF5 workflow build guide with Liquid templates
- `docs/tatari/business-incident/business-incident.md` - Business Incident spec (roles, process, backfill framework)
- `docs/reference/kb/incident-roles.md` - PD role plan tier limits
- PD incident type IDs: Base=P3U18MW, Major=PMQN99D, Security=PIDM8XQ, Managed=PEK5BWB, Business=PXVC6YG
