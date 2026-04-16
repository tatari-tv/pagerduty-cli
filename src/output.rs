use crate::cli::OutputFormat;
use serde_json::Value;
use std::io::IsTerminal;

pub fn print_value(value: &Value, format: &OutputFormat) {
    let as_json = match format {
        OutputFormat::Json => true,
        OutputFormat::Table => false,
        OutputFormat::Auto => !std::io::stdout().is_terminal(),
    };

    if as_json {
        print_json(value);
    } else {
        // Phase 1: table output falls back to pretty JSON
        // Future phases add per-resource table rendering
        print_json(value);
    }
}

fn print_json(value: &Value) {
    let s = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    println!("{}", s);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_print_json_object() {
        // Should not panic
        print_json(&json!({"id": "abc", "name": "test"}));
    }

    #[test]
    fn test_print_json_array() {
        print_json(&json!([1, 2, 3]));
    }

    #[test]
    fn test_print_value_json_format() {
        // Should not panic for explicit json format
        print_value(&json!({"key": "val"}), &OutputFormat::Json);
    }

    #[test]
    fn test_print_value_table_format() {
        print_value(&json!({"key": "val"}), &OutputFormat::Table);
    }
}
