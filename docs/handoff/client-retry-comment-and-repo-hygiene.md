# Handoff: client retry comment and repo hygiene

Branch: main

HEAD at time of writing: f2f4d2c (v0.7.8). Findings from a read-only review on 2026-10-07; nothing has been changed yet.

## Next action

Decide whether `PdClient::send` should retry 5xx. Then either add the retry or fix the doc comment at `src/client.rs:275` that claims it exists.

## Read first

1. `src/client.rs:160-190` (`send`, the 429 retry loop)
2. `src/client.rs:270-280` (`events_post` doc comment)
3. `src/client.rs:294,321,418` (the three pagination strategies)

## Defects, each with the probe that proves it

- **False comment: "429 and 5xx get exponential backoff for free."** `src/client.rs:275`. The retry loop handles 429 only, with `Retry-After` and a fixed 5s default, 3 attempts (`src/client.rs:172-186`). There is no 5xx branch and no exponential backoff anywhere. Probe: `grep -n 'is_server_error\|backoff\|5[0-9][0-9]' src/client.rs` returns only the comment line. Events v2 is the path most likely to see a transient 5xx, so the gap matters where the comment sits.
- **Junk file committed.** A 0-byte file named `file` at repo root, commit c75245f "dumb file". Probe: `git log --oneline -1 -- file; stat -c %s file`. Delete it (`rkvr rmrf` is not needed; it is tracked and empty).
- **All responses are untyped `serde_json::Value`.** Tables are hand-built in `src/output/table.rs` (1062 lines). Not a bug, but the reason table and JSON output can drift per resource.
- **`CLAUDE.md` holds only PagerDuty domain rules** (priority vs urgency). No engineering rules live in the repo; `fleet-plugins.md` is the only place that says `plugin/` is published.

## Fleet state

- `plugin/` has one skill (`skills/triage`), pinned in `tatari-skills` at v0.7.8 (current as of 2026-10-07).
- No MCP server and no `mcp-io` dependency. The skill is CLI-only. If an MCP surface is ever added, the `commands/<cmd>::run` / `mcp.rs::<cmd>_result` sharing rule and a CLI-vs-MCP parity test apply from day one.

## Suggested skills

- `shipit` after the comment fix and `git rm file` (patch bump).
