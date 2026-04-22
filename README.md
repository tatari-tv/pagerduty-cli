# pagerduty-cli (`pd`)

A Rust CLI for configuring and managing PagerDuty. Binary name: `pd`.

Covers the full instance lifecycle - teams, schedules, escalation policies, services,
integrations, event orchestration, incident types, incident workflows, and workflow
triggers. Incident roles have no public REST API and must be configured via the
PagerDuty UI.

## Install

```bash
cargo install --path .
```

## Authentication

`pd` resolves an API token in this order:

1. `--api-token <TOKEN>` CLI flag
2. `PAGERDUTY_API_TOKEN` environment variable
3. `api-token:` field in `~/.config/pagerduty-cli/pagerduty-cli.yml`

Create a token in PagerDuty: **User Settings -> API Access -> Create API User Token**.

## Configuration

Copy `pagerduty-cli.yml` from the repo root to `~/.config/pagerduty-cli/pagerduty-cli.yml`
and edit. CLI flags and env vars override anything in the config file.

## Output formats

```bash
pd --output json priority list     # pretty JSON
pd --output table priority list    # human-readable table
pd --output auto priority list     # table on a TTY, JSON when piped (default)
```

List commands have dedicated table renderers. Single-resource GETs always emit JSON.

## Logs

Written to `~/.local/share/pagerduty-cli/logs/pagerduty-cli.log`. Use
`-l debug` or `-l trace` to increase verbosity.

## Known Limitations

**Name-to-ID cache staleness on rename.** `pd` caches name → ID mappings per
PagerDuty account at `~/.cache/pd/ids/<subdomain>/` for 5 minutes. If you rename
a resource in the PD UI, the CLI may serve the old mapping until the TTL expires.
Run `pd cache clear` to force a refresh, or pass `--no-cache` for a single invocation.

---

## Commands

List commands accept zero or more positional `PATTERNS` that filter by the primary
name of each item with a 3-tier fallback: exact → starts-with → contains (OR
semantics within a tier; first non-empty tier wins).

### Raw REST Passthrough

The escape hatch for any endpoint not yet wrapped by a native command:

```bash
pd rest GET /teams
pd rest POST /incidents --body '{"incident": {...}}'
pd rest PUT /services/{id} --body '{"service": {...}}'
pd rest DELETE /maintenance_windows/{id}
```

Accepts any PagerDuty REST v2 path. Use `--output json` to pipe responses to `jq`.

---

### Priorities

| Command | Purpose |
|---------|---------|
| `pd priority list` | List configured priority levels |
| `pd priority verify` | Verify P1-P4 are correctly configured |

---

### Teams

| Command | Purpose |
|---------|---------|
| `pd team list [PATTERNS...]` | List teams |
| `pd team get <name-or-id>` | Fetch a team |
| `pd team create --name <name> [--description] [--from-file FILE] [--example]` | Create a team |
| `pd team update <name-or-id> [--name] [--description] [--from-file FILE]` | Update a team |
| `pd team delete <name-or-id>` | Delete a team |
| `pd team member list <team> [PATTERNS...]` | List team members |
| `pd team member add <team> <user> [--role observer\|responder\|manager]` | Add a user to a team |
| `pd team member remove <team> <user>` | Remove a user from a team |

---

### Users

| Command | Purpose |
|---------|---------|
| `pd user list [PATTERNS...]` | List users |
| `pd user get <email-or-id>` | Fetch a user by email or PagerDuty ID |

User roles (Admin, Responder, Observer) must be set in the PagerDuty UI by an admin.

---

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

---

### Escalation Policies

| Command | Purpose |
|---------|---------|
| `pd escalation list [PATTERNS...]` | List escalation policies |
| `pd escalation get <name-or-id>` | Fetch a policy |
| `pd escalation create [--name] [--team] [--from-file FILE] [--example]` | Create a policy |
| `pd escalation update <name-or-id> --from-file FILE` | Update a policy |
| `pd escalation delete <name-or-id>` | Delete a policy |

---

### Services

| Command | Purpose |
|---------|---------|
| `pd service list [PATTERNS...]` | List services |
| `pd service get <name-or-id>` | Fetch a service |
| `pd service create [--name] [--escalation] [--description] [--from-file FILE] [--example]` | Create a service |
| `pd service update <name-or-id> --from-file FILE` | Update a service |
| `pd service delete <name-or-id>` | Delete a service |
| `pd service integration list <service> [PATTERNS...]` | List integrations on a service |
| `pd service integration get <service> <integration-id>` | Fetch an integration |
| `pd service integration create <service> [--type] [--name] [--from-file FILE] [--example]` | Create an integration |
| `pd service integration delete <service> <integration-id>` | Delete an integration |

---

### On-Call

| Command | Purpose |
|---------|---------|
| `pd oncall list [PATTERNS...]` | Show who is currently on call (matches schedule, EP, or user name) |

---

### Maintenance Windows

| Command | Purpose |
|---------|---------|
| `pd maintenance list [PATTERNS...] [--team] [--service]` | List maintenance windows |
| `pd maintenance get <ID>` | Fetch a maintenance window |
| `pd maintenance create --service <svc>... [--start] [--end] [--description] [--from-file FILE] [--example]` | Create a maintenance window |
| `pd maintenance update <ID> [--start] [--end] [--description]` | Update a maintenance window |
| `pd maintenance delete <ID>` | Delete a maintenance window |

---

### Alert Grouping

| Command | Purpose |
|---------|---------|
| `pd alert-grouping list [PATTERNS...]` | List alert grouping settings |
| `pd alert-grouping get <ID>` | Fetch a setting |
| `pd alert-grouping create --service <svc>... [--type] [--name] [--from-file FILE] [--example]` | Create a setting |
| `pd alert-grouping update <ID> [--from-file FILE]` | Update a setting |
| `pd alert-grouping delete <ID>` | Delete a setting |

---

### Event Orchestrations

| Command | Purpose |
|---------|---------|
| `pd orchestration list [PATTERNS...]` | List event orchestrations |
| `pd orchestration get <name-or-id>` | Fetch an orchestration |
| `pd orchestration router get <orchestration>` | Fetch the orchestration's router |
| `pd orchestration router update <orchestration> --from-file FILE` | Replace the router |

---

### Incidents

| Command | Purpose |
|---------|---------|
| `pd incident list [PATTERNS...] [--status] [--priority] [--team] [--since] [--until]` | List incidents |
| `pd incident get <ID>` | Fetch an incident |
| `pd incident create --title <title> --service <svc> [--priority] [--type] [--from-file FILE] [--example]` | Create an incident |
| `pd incident update <ID> [--status] [--priority] [--type] [--title]` | Update an incident |
| `pd incident note list <ID>` | List notes on an incident |
| `pd incident note create <ID> --content <text>` | Add a note |
| `pd incident alert list <ID>` | List alerts on an incident |
| `pd incident alert get <ID> <alert-id>` | Fetch an alert |

---

### Incident Types

Incident types categorize incidents for workflow targeting, reporting, and custom fields.
PagerDuty provides default types (Base, Major, Security). Additional custom types can
be created on Business and Enterprise plans.

| Command | Purpose |
|---------|---------|
| `pd incident type list [--filter enabled\|disabled\|all]` | List incident types |
| `pd incident type get <ID\|slug\|"Display Name">` | Fetch one type |
| `pd incident type create --name <slug> --display-name <name> [--description] [--parent <type>]` | Create a new type |
| `pd incident type update <ID\|name> [--display-name] [--description] [--enabled]` | Update a type |
| `pd incident type field list <type>` | List custom fields on a type |
| `pd incident type field create <type> --name --data-type --field-type` | Add a custom field |

The `--parent` flag on `create` accepts an ID, slug, or display name and resolves it
automatically. All custom types must have a parent - typically `Base Incident`.

```bash
pd incident type create \
  --name managed_incident \
  --display-name "Managed Incident" \
  --description "Full incident response process" \
  --parent "Base Incident"
```

---

### Incident Workflows

Workflows automate incident response steps. Keep workflow definitions as YAML in
source control and use `import` to apply them. Workflows are always imported
**disabled** - enable them explicitly after testing.

| Command | Purpose |
|---------|---------|
| `pd incident workflow list [PATTERNS...]` | List workflows |
| `pd incident workflow get <ID> [--include-steps]` | Fetch a workflow (add `--include-steps` to see actions) |
| `pd incident workflow create --name <name> [--from-file FILE]` | Create a workflow |
| `pd incident workflow update <ID> [--name] [--description]` | Update a workflow |
| `pd incident workflow delete <ID>` | Delete a workflow |
| `pd incident workflow enable <ID>` | Enable a workflow |
| `pd incident workflow disable <ID>` | Disable a workflow |
| `pd incident workflow export <ID> [--real-id <ID>]` | Export to YAML for version control |
| `pd incident workflow import <FILE> [--id <ID>]` | Create or update from YAML (idempotent) |

#### Workflow YAML format

```yaml
workflow:
  name: My Workflow
  description: What this workflow does
  is-enabled: false
  steps:
    - name: Create Slack Channel
      action-id: pagerduty.com:slack:create-a-channel:4
      inputs:
        - name: Workspace
          value: T0XXXXXXX
        - name: Channel Name
          value: "inc-{{incident.created_at | date: \"%Y%m%d-%H%M\"}}"

    - name: Post to #incidents
      action-id: pagerduty.com:slack:send-markdown-message:3
      inputs:
        - name: Workspace
          value: T0XXXXXXX
        - name: Channel
          value: A specific channel
        - name: Select the Channel
          value: incidents
        - name: Message
          value: ":rotating_light: *{{incident.title}}*"

trigger:
  trigger-type: incident_type       # incident_type | conditional | manual
  incident-types:
    - managed_incident              # use the slug, not the display name
```

For a `conditional` trigger:

```yaml
trigger:
  trigger-type: conditional
  condition: "incident.incident_type.name == 'Managed Incident'"
```

Use `pd action list` to discover available action IDs and their input schemas.
Use `pd incident workflow export <ID>` to see the YAML format for an existing workflow.

**Known constraint:** The `delay` action (`pagerduty.com:incident-workflows:delay:1`)
cannot be used in the same workflow as `create-a-channel`. Add delay steps manually
via the PagerDuty UI after import.

---

### Workflow Triggers

Triggers define when workflows fire. A workflow without a trigger never runs.
`pd incident workflow import` creates the trigger automatically from the YAML `trigger:`
block - you rarely need to manage triggers directly.

| Command | Purpose |
|---------|---------|
| `pd incident trigger list [PATTERNS...]` | List all workflow triggers |
| `pd incident trigger get <ID>` | Fetch one trigger |
| `pd incident trigger create --workflow <ID> --type <conditional\|manual\|incident-type> [--condition <pcl>] [--incident-types <slug,...>]` | Create a trigger |
| `pd incident trigger update <ID> [--condition] [--incident-types]` | Update a trigger |
| `pd incident trigger delete <ID>` | Delete a trigger |
| `pd incident trigger bind <trigger-id> --service <name-or-id>` | Scope trigger to a specific service |
| `pd incident trigger unbind <trigger-id> --service <name-or-id>` | Remove service scope |

---

### Workflow Actions

Actions are the building blocks of workflow steps. Use `list` to discover available
action IDs and `get` to see the full input/output schema before writing workflow YAML.

| Command | Purpose |
|---------|---------|
| `pd action list [PATTERNS...]` | List available workflow actions |
| `pd action get <ID>` | Full input/output schema for one action |

Action IDs follow the pattern `domain:package:function:version`, for example:
`pagerduty.com:slack:create-a-channel:4`.

---

### Log Entries

| Command | Purpose |
|---------|---------|
| `pd log list [PATTERNS...] [--since] [--until]` | List log entries (operational audit trail) |
| `pd log get <ID>` | Fetch one log entry |

---

### Change Events

| Command | Purpose |
|---------|---------|
| `pd change list [PATTERNS...] [--since] [--until] [--service]` | List change events |
| `pd change get <ID>` | Fetch one change event |
| `pd change create --service <svc> [--summary] [--links] [--routing-key] [--from-file] [--example]` | Enqueue a change event via Events API v2 |

`pd change create` resolves `--service` to an Events API v2 integration on that
service and uses its routing key. Short-circuit with `--routing-key`, the
`PAGERDUTY_ROUTING_KEY` env var, or a `routing-key:` field in the config file.

---

### Cache

| Command | Purpose |
|---------|---------|
| `pd cache clear [--all-accounts]` | Clear the name-to-ID cache |

---

## Breaking changes in v0.6.0

- The top-level `pd trigger` command was removed. Use `pd incident trigger ...` instead.
- All list-command pattern filtering is now case-insensitive.
