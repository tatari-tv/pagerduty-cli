---
name: triage
description: Triage PagerDuty incidents from the terminal via the `pd` CLI - who's currently on call, list/get active incidents, acknowledge or resolve an incident, add a note to one. Use when a task involves an ongoing incident, paging, or on-call rotation ("who's on call for X", "ack that incident", "resolve it", "what's firing right now"). Scoped to incident + on-call triage only.
user-invocable: true
argument-hint: "<incident-id | schedule/team name>"
---

# pagerduty:triage

Incident and on-call triage with the `pd` CLI. This skill covers the handful of
verbs an agent actually uses during a page; it deliberately does NOT touch
PagerDuty's configuration surface (see Out of scope).

## Auth

`pd` needs a PagerDuty API token, resolved in this order: `--api-token <T>` flag
> `PAGERDUTY_API_TOKEN` env var > `api-token` in
`~/.config/pagerduty-cli/pagerduty-cli.yml`. If a call returns an auth error,
the token is missing or wrong at whichever level you expected it. Add
`--output json` to any command for machine-readable output (values: `auto`,
`json`, `table`).

## Read-only triage (safe, run freely)

```bash
pd oncall list                             # who is currently on call
pd oncall list "Site Reliability"          # filter by schedule / escalation-policy / user name
pd incident list                           # triggered+acknowledged in the last 1 day (the default)
pd incident get <id-or-number>             # full detail on one incident
pd incident note list <id>                 # existing notes on an incident
pd incident alert list <id>                # alerts attached to an incident (also: alert get <id> <alert-id>)
```

## Mutating actions (LIVE - confirm with the user first)

**`pd incident update` and `pd incident note create` mutate a real, live
incident immediately. There is NO `--yes`/confirm prompt and NO undo** - the
change hits PagerDuty the moment the command runs, and other responders see it.
Because the CLI does not gate these, YOU must: state plainly what the action
will do to which incident, get the user's explicit confirmation, and only then
run it. Never fire these speculatively while triaging.

```bash
pd incident update <id> --status acknowledged   # take ownership / silence the page
pd incident update <id> --status resolved        # close the incident
pd incident note create <id> "text"              # add an investigation note ('-' reads stdin)
```

- `--status` accepts `triggered`, `acknowledged`, or `resolved`.
- Acknowledge when you're actively working it; resolve only when it's genuinely
  done; add a note to leave a breadcrumb for other responders.

## Out of scope (not agent work)

This skill intentionally excludes PagerDuty's config-lifecycle namespaces -
these are human clickops / change-managed territory, not something an agent
should drive mid-triage:

- `pd orchestration` - event orchestrations and routers
- `pd escalation` - escalation policies
- `pd alert-grouping` - alert grouping settings
- `pd maintenance` - maintenance windows

If a task genuinely needs one of these, that's a deliberate configuration
change to make explicitly and carefully, not part of incident triage.
