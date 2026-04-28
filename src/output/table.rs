//! Table rendering for known PagerDuty response shapes.
//!
//! Each renderer targets one endpoint and knows which columns are worth
//! showing. The dispatcher first tries list envelopes (`"incidents": [...]`);
//! if none match, it checks singular envelopes (`"incident": {...}`) and
//! wraps them as a single-row list so `get` commands render consistently
//! with `list` commands. Anything with a shape we don't recognize falls
//! back to JSON in the parent module.

use serde_json::{Value, json};
use std::fmt::Write;

/// Default width used when the terminal size can't be detected
/// (non-TTY, piped, or tests).
pub const DEFAULT_WIDTH: usize = 120;

/// Dispatch on the top-level key of a PagerDuty list or get envelope.
/// Returns `Some(table_string)` when we have a renderer for the shape,
/// `None` otherwise (so the caller can fall back to JSON).
pub fn render(value: &Value, width: usize) -> Option<String> {
    if let Some(out) = render_list_envelope(value, width) {
        return Some(out);
    }
    render_single_envelope(value, width)
}

fn render_list_envelope(value: &Value, width: usize) -> Option<String> {
    let obj = value.as_object()?;
    if let Some(arr) = obj.get("priorities").and_then(|v| v.as_array()) {
        return Some(render_priorities(arr, width, &[]));
    }
    if let Some(arr) = obj.get("incidents").and_then(|v| v.as_array()) {
        return Some(render_incidents(arr, width, &[]));
    }
    if let Some(arr) = obj.get("incident_types").and_then(|v| v.as_array()) {
        return Some(render_incident_types(arr, width, &[]));
    }
    if let Some(arr) = obj.get("incident_workflows").and_then(|v| v.as_array()) {
        return Some(render_incident_workflows(arr, width, &[]));
    }
    if let Some(arr) = obj.get("triggers").and_then(|v| v.as_array()) {
        return Some(render_triggers(arr, width, &[]));
    }
    if let Some(arr) = obj.get("actions").and_then(|v| v.as_array()) {
        return Some(render_actions(arr, width, &[]));
    }
    if let Some(arr) = obj.get("users").and_then(|v| v.as_array()) {
        return Some(render_users(arr, width, &[]));
    }
    if let Some(arr) = obj.get("teams").and_then(|v| v.as_array()) {
        return Some(render_teams(arr, width, &[]));
    }
    if let Some(arr) = obj.get("members").and_then(|v| v.as_array()) {
        return Some(render_members(arr, width, &[]));
    }
    if let Some(arr) = obj.get("schedules").and_then(|v| v.as_array()) {
        return Some(render_schedules(arr, width, &[]));
    }
    if let Some(arr) = obj.get("overrides").and_then(|v| v.as_array()) {
        return Some(render_overrides(arr, width, &[]));
    }
    if let Some(arr) = obj.get("escalation_policies").and_then(|v| v.as_array()) {
        return Some(render_escalations(arr, width, &[]));
    }
    if let Some(arr) = obj.get("services").and_then(|v| v.as_array()) {
        return Some(render_services(arr, width, &[]));
    }
    if let Some(arr) = obj.get("integrations").and_then(|v| v.as_array()) {
        return Some(render_integrations(arr, width, &[]));
    }
    if let Some(arr) = obj.get("oncalls").and_then(|v| v.as_array()) {
        return Some(render_oncalls(arr, width, &[]));
    }
    if let Some(arr) = obj.get("orchestrations").and_then(|v| v.as_array()) {
        return Some(render_orchestrations(arr, width, &[]));
    }
    if let Some(arr) = obj.get("maintenance_windows").and_then(|v| v.as_array()) {
        return Some(render_maintenance_windows(arr, width, &[]));
    }
    if let Some(arr) = obj
        .get("alert_grouping_settings")
        .and_then(|v| v.as_array())
    {
        return Some(render_alert_grouping(arr, width, &[]));
    }
    if let Some(arr) = obj.get("log_entries").and_then(|v| v.as_array()) {
        return Some(render_log_entries(arr, width, &[]));
    }
    if let Some(arr) = obj.get("change_events").and_then(|v| v.as_array()) {
        return Some(render_change_events(arr, width, &[]));
    }
    if let Some(arr) = obj.get("alerts").and_then(|v| v.as_array()) {
        return Some(render_alerts(arr, width, &[]));
    }
    if let Some(arr) = obj.get("notes").and_then(|v| v.as_array()) {
        return Some(render_notes(arr, width, &[]));
    }
    None
}

/// Render function signature shared by every resource renderer: takes the rows,
/// terminal width, and an optional protected-column mask.
type RowRenderer = fn(&[Value], usize, &[bool]) -> String;

/// Handle single-resource envelopes from `get` commands (`{"incident": {...}}`)
/// by wrapping the object as a single-row list and reusing the list renderer.
/// Mapping is singular JSON key → list renderer.
fn render_single_envelope(value: &Value, width: usize) -> Option<String> {
    let obj = value.as_object()?;
    let mappings: &[(&str, RowRenderer)] = &[
        ("incident", render_incidents),
        ("incident_type", render_incident_types),
        ("incident_workflow", render_incident_workflows),
        ("trigger", render_triggers),
        ("action", render_actions),
        ("user", render_users),
        ("team", render_teams),
        ("member", render_members),
        ("schedule", render_schedules),
        ("override", render_overrides),
        ("escalation_policy", render_escalations),
        ("service", render_services),
        ("integration", render_integrations),
        ("oncall", render_oncalls),
        ("orchestration", render_orchestrations),
        ("maintenance_window", render_maintenance_windows),
        ("alert_grouping_setting", render_alert_grouping),
        ("log_entry", render_log_entries),
        ("change_event", render_change_events),
        ("alert", render_alerts),
        ("note", render_notes),
        ("priority", render_priorities),
    ];

    for (key, render_fn) in mappings {
        if let Some(v) = obj.get(*key)
            && v.is_object()
        {
            let rows = [v.clone()];
            return Some(render_fn(&rows, width, &[]));
        }
    }
    None
}

fn render_priorities(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["NAME", "DESCRIPTION"],
        rows,
        &[|r| str_field(r, "name"), |r| str_field(r, "description")],
        None,
        width,
    )
}

fn render_incidents(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &[
            "NUMBER",
            "TITLE",
            "STATUS",
            "SERVICE",
            "PRIORITY",
            "URGENCY",
            "CREATED_AT",
        ],
        rows,
        &[
            |r| {
                r.get("incident_number")
                    .and_then(|v| v.as_i64())
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            },
            |r| str_field(r, "title"),
            |r| str_field(r, "status"),
            |r| nested_str(r, "service", "summary"),
            |r| nested_str(r, "priority", "summary"),
            |r| str_field(r, "urgency"),
            |r| str_field(r, "created_at"),
        ],
        None,
        width,
    )
}

fn render_incident_types(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "DISPLAY_NAME", "ENABLED", "PARENT_ID"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| str_field(r, "display_name"),
            |r| bool_field(r, "enabled"),
            |r| nested_str(r, "parent", "id"),
        ],
        None,
        width,
    )
}

fn render_incident_workflows(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "ENABLED"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| bool_field(r, "is_enabled"),
        ],
        None,
        width,
    )
}

fn render_triggers(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "TYPE", "WORKFLOW", "INCIDENT_TYPES"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "trigger_type"),
            |r| nested_str(r, "workflow", "name"),
            |r| {
                r.get("incident_types")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default()
            },
        ],
        None,
        width,
    )
}

fn render_actions(rows: &[Value], width: usize, _: &[bool]) -> String {
    // Action IDs are long (~55 chars) but critical for copy/paste into
    // workflow definitions. Protect the ID column so DESCRIPTION soaks up
    // truncation instead.
    render_table(
        &["ID", "FUNCTION_NAME", "DESCRIPTION"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "function_name"),
            |r| str_field(r, "description"),
        ],
        Some(&[true, false, false]),
        width,
    )
}

fn render_users(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "EMAIL", "ROLE"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| str_field(r, "email"),
            |r| str_field(r, "role"),
        ],
        None,
        width,
    )
}

fn render_teams(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "DESCRIPTION"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| str_field(r, "description"),
        ],
        None,
        width,
    )
}

fn render_members(rows: &[Value], width: usize, _: &[bool]) -> String {
    // The `/teams/:id/members` payload does not include `user.email`, so the
    // EMAIL column was always blank. Dropped. Use `pd user get <id>` if you
    // need the email for a specific member.
    render_table(
        &["USER_ID", "NAME", "ROLE"],
        rows,
        &[
            |r| nested_str(r, "user", "id"),
            |r| nested_str(r, "user", "summary"),
            |r| str_field(r, "role"),
        ],
        None,
        width,
    )
}

fn render_schedules(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "TIME_ZONE", "DESCRIPTION"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| str_field(r, "time_zone"),
            |r| str_field(r, "description"),
        ],
        None,
        width,
    )
}

fn render_overrides(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "USER", "START", "END"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| nested_str(r, "user", "summary"),
            |r| str_field(r, "start"),
            |r| str_field(r, "end"),
        ],
        None,
        width,
    )
}

fn render_services(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "ESCALATION_POLICY", "STATUS"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| nested_str(r, "escalation_policy", "summary"),
            |r| str_field(r, "status"),
        ],
        None,
        width,
    )
}

fn render_integrations(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "TYPE"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| {
                r.get("name")
                    .or_else(|| r.get("summary"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            },
            |r| str_field(r, "type"),
        ],
        None,
        width,
    )
}

fn render_oncalls(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["SCHEDULE", "ESCALATION_POLICY", "USER", "LEVEL"],
        rows,
        &[
            |r| nested_str(r, "schedule", "summary"),
            |r| nested_str(r, "escalation_policy", "summary"),
            |r| nested_str(r, "user", "summary"),
            |r| {
                r.get("escalation_level")
                    .and_then(|v| v.as_i64())
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            },
        ],
        None,
        width,
    )
}

fn render_escalations(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "TEAMS", "RULES"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| {
                r.get("teams")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|t| t.get("summary").and_then(|v| v.as_str()))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default()
            },
            |r| {
                r.get("escalation_rules")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len().to_string())
                    .unwrap_or_default()
            },
        ],
        None,
        width,
    )
}

fn render_orchestrations(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "TEAM", "ROUTES"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| nested_str(r, "team", "summary"),
            |r| {
                r.get("routes")
                    .and_then(|v| v.as_i64())
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            },
        ],
        None,
        width,
    )
}

fn render_maintenance_windows(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "DESCRIPTION", "START", "END", "SERVICES"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "description"),
            |r| str_field(r, "start_time"),
            |r| str_field(r, "end_time"),
            |r| {
                r.get("services")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.get("summary").and_then(|v| v.as_str()))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default()
            },
        ],
        None,
        width,
    )
}

fn render_alert_grouping(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "NAME", "TYPE", "SERVICES"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| str_field(r, "type"),
            |r| {
                r.get("services")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| {
                                s.get("name")
                                    .or_else(|| s.get("summary"))
                                    .and_then(|v| v.as_str())
                            })
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default()
            },
        ],
        None,
        width,
    )
}

fn render_log_entries(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "TYPE", "INCIDENT", "SUMMARY", "CREATED_AT"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "type"),
            |r| nested_str(r, "incident", "summary"),
            |r| str_field(r, "summary"),
            |r| str_field(r, "created_at"),
        ],
        None,
        width,
    )
}

fn render_change_events(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "SUMMARY", "SERVICE", "SOURCE", "TIMESTAMP"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "summary"),
            |r| nested_str(r, "service", "summary"),
            |r| str_field(r, "source"),
            |r| str_field(r, "timestamp"),
        ],
        None,
        width,
    )
}

fn render_alerts(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &[
            "ID",
            "STATUS",
            "SEVERITY",
            "SUMMARY",
            "SERVICE",
            "CREATED_AT",
        ],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "status"),
            |r| str_field(r, "severity"),
            |r| str_field(r, "summary"),
            |r| nested_str(r, "service", "summary"),
            |r| str_field(r, "created_at"),
        ],
        None,
        width,
    )
}

fn render_notes(rows: &[Value], width: usize, _: &[bool]) -> String {
    render_table(
        &["ID", "USER", "CONTENT", "CREATED_AT"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| nested_str(r, "user", "summary"),
            |r| str_field(r, "content"),
            |r| str_field(r, "created_at"),
        ],
        None,
        width,
    )
}

// ---------------------------------------------------------------------------
// Generic table rendering
// ---------------------------------------------------------------------------

type FieldFn = fn(&Value) -> String;

/// Render a table. `protect` optionally marks columns whose natural width
/// should be preserved. Protected columns are excluded from the shrink loop
/// until all unprotected columns have been shrunk down to at least 1 char.
fn render_table(
    headers: &[&str],
    rows: &[Value],
    fields: &[FieldFn],
    protect: Option<&[bool]>,
    width: usize,
) -> String {
    let mut grid: Vec<Vec<String>> = Vec::with_capacity(rows.len() + 1);
    grid.push(headers.iter().map(|s| s.to_string()).collect());
    for row in rows {
        grid.push(fields.iter().map(|f| f(row)).collect());
    }

    // Natural widths per column.
    let cols = headers.len();
    let mut widths = vec![0usize; cols];
    for r in &grid {
        for (i, cell) in r.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    let protected: Vec<bool> = match protect {
        Some(p) if p.len() == cols => p.to_vec(),
        _ => vec![false; cols],
    };

    // Shrink to fit `width`. Two spaces between columns. Shrink whichever
    // column is currently widest one character at a time, so long middle
    // columns are truncated instead of silently line-wrapping past the
    // terminal edge. Protected columns only shrink when every unprotected
    // column is already at 1.
    let sep = "  ";
    let sep_total = sep.len() * cols.saturating_sub(1);
    loop {
        let total: usize = widths.iter().sum::<usize>() + sep_total;
        if total <= width {
            break;
        }
        // Prefer shrinking an unprotected column with width > 1.
        let unprotected_target = widths
            .iter()
            .enumerate()
            .filter(|(i, w)| !protected[*i] && **w > 1)
            .max_by_key(|(_, w)| **w);
        let idx = match unprotected_target {
            Some((i, _)) => i,
            None => {
                // All unprotected columns shrunk to the minimum. Start
                // trimming the widest protected column if any still have room.
                let widest = widths
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, w)| **w)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                if widths[widest] == 0 {
                    break;
                }
                widest
            }
        };
        widths[idx] -= 1;
    }

    let mut out = String::new();
    for r in &grid {
        for (i, cell) in r.iter().enumerate() {
            let truncated = truncate(cell, widths[i]);
            let pad = widths[i].saturating_sub(truncated.chars().count());
            if i > 0 {
                out.push_str(sep);
            }
            let _ = write!(out, "{}{}", truncated, " ".repeat(pad));
        }
        out.push('\n');
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".repeat(max);
    }
    let mut result: String = s.chars().take(max - 1).collect();
    result.push('…');
    result
}

fn str_field(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn bool_field(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(|v| v.as_bool())
        .map(|b| b.to_string())
        .unwrap_or_default()
}

fn nested_str(row: &Value, outer: &str, inner: &str) -> String {
    row.get(outer)
        .and_then(|v| v.get(inner))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

// Avoid warnings if `json!` unused in some build configurations.
#[allow(dead_code)]
fn _use_json_macro_marker() -> Value {
    json!({})
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn render_priorities_shows_name_and_description() {
        let v = json!({
            "priorities": [
                {"name": "P1", "description": "Critical"},
                {"name": "P2", "description": "Major impact"}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("NAME"));
        assert!(out.contains("DESCRIPTION"));
        assert!(out.contains("P1"));
        assert!(out.contains("Critical"));
        assert!(out.contains("P2"));
    }

    #[test]
    fn render_incident_types_shows_parent_id() {
        let v = json!({
            "incident_types": [
                {"id": "T1", "name": "default", "display_name": "Base", "enabled": true, "parent": null},
                {"id": "T2", "name": "managed", "display_name": "Managed", "enabled": true,
                 "parent": {"id": "T1", "type": "incident_type_reference"}}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("PARENT_ID"));
        assert!(out.contains("T1"));
        assert!(out.contains("T2"));
        let lines: Vec<&str> = out.lines().collect();
        assert!(!lines[1].contains("null"));
    }

    #[test]
    fn render_workflows_shows_is_enabled() {
        let v = json!({
            "incident_workflows": [
                {"id": "WF1", "name": "Managed Response", "is_enabled": true},
                {"id": "WF2", "name": "Visibility", "is_enabled": false}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("WF1"));
        assert!(out.contains("true"));
        assert!(out.contains("false"));
    }

    #[test]
    fn render_triggers_joins_incident_types() {
        let v = json!({
            "triggers": [
                {"id": "TR1", "trigger_type": "incident_type",
                 "workflow": {"id": "WF1", "name": "Managed"},
                 "incident_types": ["Managed Incident"]},
                {"id": "TR2", "trigger_type": "conditional",
                 "workflow": {"id": "WF2", "name": "Visibility"}}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("TR1"));
        assert!(out.contains("Managed Incident"));
        assert!(out.contains("conditional"));
    }

    #[test]
    fn render_actions_includes_function_name_and_description() {
        let v = json!({
            "actions": [
                {"id": "pagerduty.aws:asg", "function_name": "auto-scaling-set", "description": "Protect from scale-in"}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("pagerduty.aws:asg"));
        assert!(out.contains("auto-scaling-set"));
    }

    #[test]
    fn render_actions_preserves_long_ids_at_narrow_width() {
        // Regression for bug 3: action IDs up to ~55 chars must survive
        // truncation; DESCRIPTION should take the hit instead.
        let long_id = "pagerduty.com:incident-workflows:add-conference-bridge:5";
        let v = json!({
            "actions": [
                {"id": long_id, "function_name": "add-conference-bridge",
                 "description": "Adds a phone number and/or URL to an incident as a conference bridge."}
            ]
        });
        let out = render(&v, 100).unwrap();
        assert!(
            out.contains(long_id),
            "long ID should be preserved; output was:\n{}",
            out
        );
    }

    #[test]
    fn render_users_shows_email_and_role() {
        let v = json!({
            "users": [
                {"id": "U1", "name": "Scott Idler", "email": "scott.idler@tatari.tv", "role": "admin"}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("scott.idler@tatari.tv"));
        assert!(out.contains("admin"));
    }

    #[test]
    fn render_members_drops_empty_email_column() {
        // Regression for bug 4: /teams/:id/members doesn't return email,
        // so we render only USER_ID / NAME / ROLE.
        let v = json!({
            "members": [
                {"user": {"id": "U1", "summary": "Keegan Ferrando"}, "role": "manager"}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("USER_ID"));
        assert!(out.contains("NAME"));
        assert!(out.contains("ROLE"));
        assert!(!out.contains("EMAIL"), "EMAIL column should be dropped");
        assert!(out.contains("Keegan Ferrando"));
    }

    #[test]
    fn render_incidents_list() {
        let v = json!({
            "incidents": [
                {
                    "incident_number": 334,
                    "title": "DatasourceNoData",
                    "status": "resolved",
                    "urgency": "high",
                    "created_at": "2026-04-15T07:20:33Z",
                    "service": {"summary": "TVP"},
                    "priority": {"summary": "P2"}
                }
            ]
        });
        let out = render(&v, 200).unwrap();
        assert!(out.contains("NUMBER"));
        assert!(out.contains("334"));
        assert!(out.contains("DatasourceNoData"));
        assert!(out.contains("resolved"));
        assert!(out.contains("TVP"));
        assert!(out.contains("P2"));
    }

    #[test]
    fn render_incident_get_single_envelope() {
        // Regression for bug 2: `get` commands returned raw JSON. Singular
        // envelopes should now render as a 1-row table.
        let v = json!({
            "incident": {
                "incident_number": 42,
                "title": "Test",
                "status": "resolved",
                "urgency": "low",
                "created_at": "2026-04-15T00:00:00Z",
                "service": {"summary": "svc"},
                "priority": null
            }
        });
        let out = render(&v, 200).unwrap();
        assert!(out.contains("NUMBER"));
        assert!(out.contains("42"));
        assert!(out.contains("Test"));
    }

    #[test]
    fn render_user_get_single_envelope() {
        let v = json!({
            "user": {"id": "U1", "name": "Scott", "email": "s@example.com", "role": "admin"}
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("Scott"));
        assert!(out.contains("s@example.com"));
    }

    #[test]
    fn render_orchestration_get_single_envelope() {
        let v = json!({
            "orchestration": {
                "id": "O1",
                "name": "airflow-test",
                "routes": 1,
                "team": {"summary": "SRE"}
            }
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("airflow-test"));
        assert!(out.contains("SRE"));
    }

    #[test]
    fn render_orchestrations_list() {
        let v = json!({
            "orchestrations": [
                {"id": "O1", "name": "alpha", "routes": 1, "team": {"summary": "SRE"}},
                {"id": "O2", "name": "beta", "routes": 2, "team": null}
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("alpha"));
        assert!(out.contains("beta"));
    }

    #[test]
    fn render_log_entries_list() {
        let v = json!({
            "log_entries": [
                {
                    "id": "L1",
                    "type": "trigger_log_entry",
                    "summary": "Triggered through the API.",
                    "created_at": "2026-04-17T00:00:00Z",
                    "incident": {"summary": "[#1] thing"}
                }
            ]
        });
        let out = render(&v, 200).unwrap();
        assert!(out.contains("L1"));
        assert!(out.contains("trigger_log_entry"));
        assert!(out.contains("[#1] thing"));
    }

    #[test]
    fn render_maintenance_windows_list() {
        let v = json!({
            "maintenance_windows": [
                {
                    "id": "M1",
                    "description": "DB upgrade",
                    "start_time": "2026-04-01T00:00:00Z",
                    "end_time": "2026-04-01T01:00:00Z",
                    "services": [{"summary": "Platform"}, {"summary": "API"}]
                }
            ]
        });
        let out = render(&v, 200).unwrap();
        assert!(out.contains("M1"));
        assert!(out.contains("DB upgrade"));
        assert!(out.contains("Platform,API"));
    }

    #[test]
    fn render_alert_grouping_list() {
        let v = json!({
            "alert_grouping_settings": [
                {
                    "id": "AG1", "name": "TVP", "type": "intelligent",
                    "services": [{"name": "TVP"}]
                }
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("AG1"));
        assert!(out.contains("intelligent"));
        assert!(out.contains("TVP"));
    }

    #[test]
    fn render_change_events_list() {
        let v = json!({
            "change_events": [
                {
                    "id": "C1", "summary": "Deploy",
                    "source": "ci",
                    "timestamp": "2026-04-17T10:00:00Z",
                    "service": {"summary": "Platform"}
                }
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("C1"));
        assert!(out.contains("Deploy"));
        assert!(out.contains("Platform"));
    }

    #[test]
    fn render_alerts_list() {
        let v = json!({
            "alerts": [
                {
                    "id": "A1",
                    "status": "resolved",
                    "severity": "critical",
                    "summary": "Disk full",
                    "created_at": "2026-04-17T00:00:00Z",
                    "service": {"summary": "Platform"}
                }
            ]
        });
        let out = render(&v, 200).unwrap();
        assert!(out.contains("A1"));
        assert!(out.contains("resolved"));
        assert!(out.contains("Disk full"));
    }

    #[test]
    fn render_notes_list() {
        let v = json!({
            "notes": [
                {
                    "id": "N1",
                    "content": "Investigating",
                    "created_at": "2026-04-17T00:00:00Z",
                    "user": {"summary": "Scott Idler"}
                }
            ]
        });
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("N1"));
        assert!(out.contains("Investigating"));
        assert!(out.contains("Scott Idler"));
    }

    #[test]
    fn render_returns_none_for_unknown_shape() {
        let v = json!({"something_else": [{"x": 1}]});
        assert!(render(&v, DEFAULT_WIDTH).is_none());
    }

    #[test]
    fn render_handles_empty_list() {
        let v = json!({"priorities": []});
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        assert!(out.contains("NAME"));
    }

    #[test]
    fn truncate_uses_ellipsis() {
        assert_eq!(truncate("hello world", 20), "hello world");
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("hi", 2), "hi");
    }

    #[test]
    fn narrow_width_fits_within_budget() {
        let v = json!({
            "priorities": [
                {"name": "P1", "description": "A very long description that will not fit"}
            ]
        });
        let out = render(&v, 30).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        for line in lines {
            assert!(
                line.chars().count() <= 30,
                "line too wide ({}): {:?}",
                line.chars().count(),
                line
            );
        }
    }

    #[test]
    fn narrow_width_shrinks_widest_column_not_last() {
        let long_name: String = "X".repeat(150);
        let v = json!({
            "incident_workflows": [
                {"id": "WF1", "name": long_name, "is_enabled": true}
            ]
        });
        let out = render(&v, 60).unwrap();
        for line in out.lines() {
            assert!(
                line.chars().count() <= 60,
                "line too wide ({}): {:?}",
                line.chars().count(),
                line
            );
        }
        assert!(
            out.contains("true"),
            "ENABLED column should not be squeezed to nothing; output was:\n{}",
            out
        );
    }
}
