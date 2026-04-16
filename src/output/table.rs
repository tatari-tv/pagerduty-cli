//! Table rendering for known PagerDuty list-response shapes.
//!
//! Each renderer targets one endpoint and knows which columns are worth
//! showing. Anything with a shape we don't recognize falls back to JSON in
//! the parent module.

use serde_json::Value;
use std::fmt::Write;

/// Default width used when the terminal size can't be detected
/// (non-TTY, piped, or tests).
pub const DEFAULT_WIDTH: usize = 120;

/// Dispatch on the top-level key of a PagerDuty list envelope. Returns
/// `Some(table_string)` when we have a renderer for the shape, `None`
/// otherwise (so the caller can fall back to JSON).
pub fn render(value: &Value, width: usize) -> Option<String> {
    let obj = value.as_object()?;
    if let Some(arr) = obj.get("priorities").and_then(|v| v.as_array()) {
        return Some(render_priorities(arr, width));
    }
    if let Some(arr) = obj.get("incident_types").and_then(|v| v.as_array()) {
        return Some(render_incident_types(arr, width));
    }
    if let Some(arr) = obj.get("incident_workflows").and_then(|v| v.as_array()) {
        return Some(render_incident_workflows(arr, width));
    }
    if let Some(arr) = obj.get("triggers").and_then(|v| v.as_array()) {
        return Some(render_triggers(arr, width));
    }
    if let Some(arr) = obj.get("actions").and_then(|v| v.as_array()) {
        return Some(render_actions(arr, width));
    }
    None
}

fn render_priorities(rows: &[Value], width: usize) -> String {
    render_table(
        &["NAME", "DESCRIPTION"],
        rows,
        &[|r| str_field(r, "name"), |r| str_field(r, "description")],
        width,
    )
}

fn render_incident_types(rows: &[Value], width: usize) -> String {
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
        width,
    )
}

fn render_incident_workflows(rows: &[Value], width: usize) -> String {
    render_table(
        &["ID", "NAME", "ENABLED"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "name"),
            |r| bool_field(r, "is_enabled"),
        ],
        width,
    )
}

fn render_triggers(rows: &[Value], width: usize) -> String {
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
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(","))
                    .unwrap_or_default()
            },
        ],
        width,
    )
}

fn render_actions(rows: &[Value], width: usize) -> String {
    render_table(
        &["ID", "FUNCTION_NAME", "DESCRIPTION"],
        rows,
        &[
            |r| str_field(r, "id"),
            |r| str_field(r, "function_name"),
            |r| str_field(r, "description"),
        ],
        width,
    )
}

// ---------------------------------------------------------------------------
// Generic table rendering
// ---------------------------------------------------------------------------

type FieldFn = fn(&Value) -> String;

fn render_table(headers: &[&str], rows: &[Value], fields: &[FieldFn], width: usize) -> String {
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

    // Shrink to fit `width`. Two spaces between columns. Shrink whichever
    // column is currently widest one character at a time, so long middle
    // columns are truncated instead of silently line-wrapping past the
    // terminal edge.
    let sep = "  ";
    let sep_total = sep.len() * cols.saturating_sub(1);
    while cols > 0 && widths.iter().sum::<usize>() + sep_total > width {
        let (widest_idx, widest_w) = widths
            .iter()
            .enumerate()
            .max_by_key(|(_, w)| **w)
            .expect("cols > 0 ensures a max");
        if *widest_w == 0 {
            break;
        }
        widths[widest_idx] -= 1;
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
        // First row has null parent - column should render blank, not "null"
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
    fn render_returns_none_for_unknown_shape() {
        let v = json!({"something_else": [{"x": 1}]});
        assert!(render(&v, DEFAULT_WIDTH).is_none());
    }

    #[test]
    fn render_handles_empty_list() {
        let v = json!({"priorities": []});
        let out = render(&v, DEFAULT_WIDTH).unwrap();
        // Header still renders
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
        // Regression: when a middle column is much wider than the last,
        // shrinking only the last column leaves the middle one overflowing
        // and the row line-wraps past the terminal edge. The renderer must
        // shrink the widest column (middle here), not unconditionally the last.
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
        // ENABLED column (last, natural width ~7) must not saturate to zero:
        // "true" should still be readable in the data row.
        assert!(
            out.contains("true"),
            "ENABLED column should not be squeezed to nothing; output was:\n{}",
            out
        );
    }
}
