# pagerduty-cli (`pd`)

A Rust CLI for configuring and managing PagerDuty. Binary name: `pd`.

Covers the full instance setup lifecycle - teams, schedules, escalation policies,
services, integrations, event orchestration, incident types, and workflows.
The only thing it cannot do is manage user roles (those require the PagerDuty UI).

## Install

```bash
cargo install --path .
```

## Authentication

`pd` resolves an API token in this order:

1. `--api-token <TOKEN>` CLI flag
2. `PAGERDUTY_API_TOKEN` environment variable (preferred in shells)
3. `api-token:` field in `~/.config/pagerduty-cli/pagerduty-cli.yml`

The secrets-repo file that populates the env var in this workspace is
`pagerduty-api-token.age` (under `scottidler/secrets`).

Create a token in PagerDuty: **User Settings -> API Access -> Create API User Token**.

## Sample config

Copy `pagerduty-cli.yml` from the repo root to `~/.config/pagerduty-cli/pagerduty-cli.yml`
and edit. CLI flags and env vars override anything set there.

## Commands

List commands accept zero or more positional `PATTERNS` that filter by the
primary name of each item with a 3-tier fallback: exact -> starts-with ->
contains (OR semantics within a tier; first non-empty tier wins).

### Priorities

| Command | Purpose |
|---------|---------|
| `pd priority list` | List P1-P5 priorities |
| `pd priority verify` | Verify P1-P4 match the Tatari severity matrix |

### Teams

| Command | Purpose |
|---------|---------|
| `pd team list [PATTERNS...]` | List teams |
| `pd team get <name-or-id>` | Fetch a team |
| `pd team create --name <name> [--description] [--from-file FILE] [--example]` | Create a team |
| `pd team update <name-or-id> [--name] [--description] [--from-file FILE]` | Update a team |
| `pd team delete <name-or-id>` | Delete a team |
| `pd team member list <team> [PATTERNS...]` | List team members |
| `pd team member add <team> <user> [--role observer\|responder\|manager]` | Add a user |
| `pd team member remove <team> <user>` | Remove a user |

### Users

| Command | Purpose |
|---------|---------|
| `pd user list [PATTERNS...]` | List users |
| `pd user get <email-or-id>` | Fetch a user by email or PagerDuty ID |

### Schedules

| Command | Purpose |
|---------|---------|
| `pd schedule list [PATTERNS...]` | List schedules |
| `pd schedule get <name-or-id>` | Fetch a schedule |
| `pd schedule create [--name] [--timezone] [--from-file FILE] [--example]` | Create a schedule |
| `pd schedule update <name-or-id> --from-file FILE` | Update a schedule |
| `pd schedule delete <name-or-id>` | Delete a schedule |
| `pd schedule override list <schedule> [--since] [--until]` | List overrides |
| `pd schedule override create <schedule> --user --start --end` | Add an override |
| `pd schedule override delete <schedule> <override-id>` | Delete an override |

### Escalation Policies

| Command | Purpose |
|---------|---------|
| `pd escalation list [PATTERNS...]` | List escalation policies |
| `pd escalation get <name-or-id>` | Fetch a policy |
| `pd escalation create [--name] [--team] [--from-file FILE] [--example]` | Create a policy |
| `pd escalation update <name-or-id> --from-file FILE` | Update a policy |
| `pd escalation delete <name-or-id>` | Delete a policy |

### Services

| Command | Purpose |
|---------|---------|
| `pd service list [PATTERNS...]` | List services |
| `pd service get <name-or-id>` | Fetch a service |
| `pd service create [--name] [--escalation] [--description] [--from-file FILE] [--example]` | Create a service |
| `pd service update <name-or-id> --from-file FILE` | Update a service |
| `pd service delete <name-or-id>` | Delete a service |
| `pd service integration list <service> [PATTERNS...]` | List integrations |
| `pd service integration get <service> <integration-id>` | Fetch an integration |
| `pd service integration create <service> [--type] [--name] [--from-file FILE] [--example]` | Create an integration |
| `pd service integration delete <service> <integration-id>` | Delete an integration |

### On-Call

| Command | Purpose |
|---------|---------|
| `pd oncall list [PATTERNS...]` | Who is currently on call (matches schedule/EP/user name) |

### Incident Types

| Command | Purpose |
|---------|---------|
| `pd incident type list [--filter enabled\|disabled\|all]` | List incident types |
| `pd incident type get <ID\|slug\|"Display Name">` | Fetch one type |
| `pd incident type create --name <slug> --display-name <name>` | Create a new type |
| `pd incident type update <ID\|name> [--display-name] [--enabled]` | Update a type |
| `pd incident type field list <type>` | List custom fields on a type |
| `pd incident type field create <type> --name --data-type --field-type` | Add a custom field |

### Incident Workflows

| Command | Purpose |
|---------|---------|
| `pd incident workflow list [--query Q]` | List workflows |
| `pd incident workflow get <ID> [--include-steps]` | Fetch a workflow |
| `pd incident workflow create --name <name> [--from-file FILE]` | Create a workflow |
| `pd incident workflow update <ID> [--name] [--description]` | Update a workflow |
| `pd incident workflow delete <ID>` | Delete a workflow |
| `pd incident workflow enable <ID>` | Enable a workflow |
| `pd incident workflow disable <ID>` | Disable a workflow |
| `pd incident workflow export <ID> [--real-id <ID>]` | Export to YAML |
| `pd incident workflow import <FILE> [--id <ID>]` | Create/update from YAML |

### Workflow Triggers

| Command | Purpose |
|---------|---------|
| `pd trigger list` | List all workflow triggers |
| `pd trigger get <ID>` | Fetch one trigger |
| `pd trigger create --workflow-id <ID> --type <conditional\|manual\|incident-type>` | Create a trigger |
| `pd trigger update <ID> [--condition] [--incident-types]` | Update a trigger |
| `pd trigger delete <ID>` | Delete a trigger |
| `pd trigger create-for-service <trigger-id> --service-id <ID>` | Bind trigger to a service |
| `pd trigger remove-from-service <trigger-id> --service-id <ID>` | Unbind trigger from service |

### Workflow Actions

| Command | Purpose |
|---------|---------|
| `pd action list [--query Q]` | List available workflow actions |
| `pd action get <ID>` | Full schema for one action |

### Raw REST Passthrough

```bash
pd rest <METHOD> <PATH> [--body JSON]
```

Covers any PagerDuty REST endpoint not yet wrapped by a native command - teams,
users, schedules, escalation policies, services, integrations, event orchestration,
and more.

Run `pd <command> --help` for flags and options.

## Output formats

```bash
pd --output json priority list     # pretty JSON
pd --output table priority list    # human-readable table
pd --output auto priority list     # table on a TTY, JSON when piped (default)
```

The list commands have dedicated table renderers. Single-resource GETs always emit JSON.

## Logs

Written to `~/.local/share/pagerduty-cli/logs/pagerduty-cli.log`. Use
`-l debug` (or `-l trace`) to increase verbosity.

---

## Instance Setup

This section walks through configuring a PagerDuty instance from scratch following the
[Tatari Incident Management](https://tatari.atlassian.net/wiki/spaces/INC) model.

PagerDuty objects have hard dependencies. Create them in this order:

```
Teams -> Users -> Schedules -> Escalation Policies -> Services -> Integrations
-> Event Orchestration -> Priorities (verify) -> Incident Types -> Workflows -> Triggers
```

Objects in the first row are infrastructure - at Tatari these are managed in
[terraform-pagerduty](https://github.com/tatari-tv/terraform-pagerduty). Use
`pd rest` to inspect or make operational changes outside of Terraform cycles.

### 1. Teams

```bash
pd rest GET /teams
pd rest POST /teams --body '{"team": {"name": "Platform", "description": "SRE Platform"}}'
pd rest PUT /teams/{team_id}/users/{user_id}   # add user (roles set in PD UI)
```

### 2. Users

Users are provisioned via SSO/SCIM. Use `pd rest` to look up IDs needed for
schedules and escalation policies:

```bash
pd rest GET /users
pd rest GET /users?query=scott
```

User roles (Admin, Responder, Observer) must be set in the PagerDuty UI by an admin.

### 3. Schedules

Tatari uses a 1-week rotation. The standard chain has a primary and secondary layer.

```bash
pd rest GET /schedules
pd rest GET /schedules/{id}

# Add a one-off override (swap on-call for a specific window)
pd rest POST /schedules/{id}/overrides \
  --body '{"override": {"start": "...", "end": "...", "user": {"id": "...", "type": "user_reference"}}}'
```

### 4. Escalation Policies

Tatari's mandatory chain: on-call -> secondary -> manager -> director -> VP.
Default delays: immediate, 15 min, 30 min, 60 min, 90 min.
See [Policy](https://tatari.atlassian.net/wiki/spaces/INC/pages/2210562055) Section 6.

```bash
pd rest GET /escalation_policies
pd rest GET /escalation_policies/{id}
```

### 5. Services

Services are the ownership unit - every incident belongs to exactly one service.
See [PagerDuty Design Guide](https://tatari.atlassian.net/wiki/spaces/INC/pages/2275147825)
for when to use 1:1 vs shared ingress services.

```bash
pd rest GET /services
pd rest GET /services/{id}
```

### 6. Integrations

Each service has one or more integrations wired to monitoring tools (Datadog,
Prometheus, Rollbar, etc.). Integration keys are stored in AWS Secrets Manager.

```bash
pd rest GET /services/{service_id}/integrations
pd rest GET /services/{service_id}/integrations/{integration_id}
```

### 7. Event Orchestration

The routing brain - signals arrive at an ingress service and are routed to owning
services by metadata and tags.

```bash
pd rest GET /event_orchestrations
pd rest GET /event_orchestrations/{id}/router
```

### 8. Priorities

Verify the P1-P4 priorities match the
[Severity Matrix](https://tatari.atlassian.net/wiki/spaces/INC/pages/2210299922):

```bash
pd priority verify
```

P1/P2 page 24/7 at high urgency. P3/P4 notify at low urgency (Slack forwarding only).

### 9. Incident Types

Incident types categorize incidents for reporting, workflow targeting, and custom fields.

```bash
pd incident type list
pd incident type create --name security-incident --display-name "Security Incident"
pd incident type field create "Security Incident" \
  --name affected-systems --data-type string --field-type field
```

### 10. Incident Workflows

Workflows automate response steps: Slack channel creation, role paging, stakeholder
notifications. Keep workflow definitions in source control as YAML.

```bash
# Export existing workflows to YAML for version control
pd incident workflow export <ID>

# Import (create or update) from YAML
pd incident workflow import workflow.yml

# Enable a workflow
pd incident workflow enable <ID>
```

### 11. Workflow Triggers

Triggers define when workflows fire (incident creation, priority change, etc.).
Bind triggers to specific services to scope their effect.

```bash
pd trigger list
pd trigger create --workflow-id <ID> --type conditional --condition "incident.priority matches 'P1'"
pd trigger create-for-service <trigger-id> --service-id <service-id>
```

---

### Related documentation

- [Tatari Incident Management & Observability](https://tatari.atlassian.net/wiki/spaces/INC)
- [PagerDuty Mental Model](https://tatari.atlassian.net/wiki/spaces/INC/pages/2275180839)
- [PagerDuty Design Guide](https://tatari.atlassian.net/wiki/spaces/INC/pages/2275147825)
- [PagerDuty Setup Guide](https://tatari.atlassian.net/wiki/spaces/INC/pages/2275082290)
- [Severity Matrix](https://tatari.atlassian.net/wiki/spaces/INC/pages/2210299922)
- [Policy](https://tatari.atlassian.net/wiki/spaces/INC/pages/2210562055)
