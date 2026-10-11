//! Rules module - additional rule definitions and helpers

use crate::config::{CommandType, RuleType, ValidationRule};

pub fn default_security_rules() -> Vec<ValidationRule> {
    vec![
        ValidationRule {
            name: "block_dangerous_functions".to_string(),
            rule_type: RuleType::RegexBlock,
            blocking: true,
            message: Some("Dangerous function calls are not allowed".to_string()),
            applies_to: vec![CommandType::All],
            config: serde_json::json!({
                "pattern": r"(?i)(pg_read_file|pg_write_file|pg_ls_dir|pg_sleep|lo_import|lo_export)"
            }),
        },
        ValidationRule {
            name: "limit_complexity".to_string(),
            rule_type: RuleType::ComplexityLimit,
            blocking: true,
            message: Some("Query is too complex".to_string()),
            applies_to: vec![CommandType::Select],
            config: serde_json::json!({ "max_joins": 10, "max_subqueries": 5 }),
        },
    ]
}

pub fn query_references_table(query: &str, table_name: &str) -> bool {
    let qu = query.to_uppercase();
    let tu = table_name.to_uppercase();
    qu.contains(&format!(" {} ", tu))
        || qu.contains(&format!(" {}.", tu))
        || qu.contains(&format!(".{} ", tu))
}

pub fn extract_table_names(query: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let qu = query.to_uppercase();

    if let Some(pos) = qu.find(" FROM ") {
        let after = &query[pos + 6..];
        if let Some(table) = after.split(|c: char| c.is_whitespace() || c == ',' || c == '(').next() {
            if !table.is_empty() { tables.push(table.to_string()); }
        }
    }
    tables
}

pub fn count_statements(query: &str) -> usize {
    let mut count = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    for ch in query.chars() {
        if in_string {
            if ch == string_char { in_string = false; }
        } else {
            if ch == '\'' || ch == '"' { in_string = true; string_char = ch; }
            else if ch == ';' { count += 1; }
        }
    }
    if !query.trim().is_empty() && count == 0 { 1 } else { count }
}