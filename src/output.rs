use crate::cli::OutputFormat;
use serde_json::Value;
use std::io::IsTerminal;

mod table;

pub fn print_value(value: &Value, format: &OutputFormat) {
    let as_json = match format {
        OutputFormat::Json => true,
        OutputFormat::Table => false,
        // Auto: JSON when piped, table when interactive.
        OutputFormat::Auto => !std::io::stdout().is_terminal(),
    };

    if as_json {
        print_json(value);
        return;
    }

    // Table mode. Dispatch on the envelope's top-level key; fall back to JSON
    // if the shape is unknown (e.g., single-resource GETs).
    let width = detect_width();
    match table::render(value, width) {
        Some(rendered) => print!("{}", rendered),
        None => print_json(value),
    }
}

fn print_json(value: &Value) {
    let s = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    println!("{}", s);
}

fn detect_width() -> usize {
    use terminal_size::{Width, terminal_size};
    if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        table::DEFAULT_WIDTH
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_print_json_object() {
        print_json(&json!({"id": "abc", "name": "test"}));
    }

    #[test]
    fn test_print_json_array() {
        print_json(&json!([1, 2, 3]));
    }

    #[test]
    fn test_print_value_json_format() {
        print_value(&json!({"key": "val"}), &OutputFormat::Json);
    }

    #[test]
    fn test_print_value_table_format() {
        print_value(&json!({"key": "val"}), &OutputFormat::Table);
    }

    #[test]
    fn test_print_value_table_known_shape() {
        print_value(
            &json!({"priorities": [{"name": "P1", "description": "crit"}]}),
            &OutputFormat::Table,
        );
    }
}
