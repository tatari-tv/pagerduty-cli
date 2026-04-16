# pagerduty-cli (`pd`)

A Rust CLI for PagerDuty incident management: priorities, incident types,
workflows, triggers, and workflow actions. Binary name: `pd`.

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

Create a token in PagerDuty: **User Settings → API Access → Create API User Token**.

## Sample config

Copy `pagerduty-cli.yml` from the repo root to `~/.config/pagerduty-cli/pagerduty-cli.yml`
and edit. CLI flags and env vars override anything set there.

## Commands

| Command | Purpose |
|---------|---------|
| `pd priority list` | List P1-P5 priorities |
| `pd priority verify` | Verify P1-P4 match the Tatari severity matrix |
| `pd incident-type list [--filter enabled\|disabled\|all]` | List incident types |
| `pd incident-type get <ID\|slug\|"Display Name">` | Fetch one type (resolves by ID, slug, or display name) |
| `pd incident-type field list <type>` | List custom fields on a type |
| `pd incident-workflow list [--query Q]` | List workflows |
| `pd incident-workflow get <ID> [--include-steps]` | Fetch a workflow |
| `pd incident-workflow export <ID> [--real-id <ID>]` | Export to YAML (with shadow-workflow fallback) |
| `pd incident-workflow import <FILE>` | Create/update workflow from YAML |
| `pd trigger list` | List all workflow triggers |
| `pd trigger get <ID>` | Fetch one trigger |
| `pd action list [--query Q]` | List available workflow actions |
| `pd action get <ID>` | Full schema for one action |
| `pd rest <METHOD> <PATH> [--body JSON]` | Raw PagerDuty REST passthrough |

Run `pd <command> --help` for flags and options.

## Output formats

```bash
pd --output json priority list     # pretty JSON
pd --output table priority list    # human-readable table
pd --output auto priority list     # table on a TTY, JSON when piped (default)
```

The five list endpoints have dedicated table renderers. Single-resource GETs
always emit JSON.

## Logs

Written to `~/.local/share/pagerduty-cli/logs/pagerduty-cli.log`. Use
`-l debug` (or `-l trace`) to increase verbosity.
