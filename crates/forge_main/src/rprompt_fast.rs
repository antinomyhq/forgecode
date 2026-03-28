//! Fast rprompt data fetcher using direct SQLite access.
//!
//! This module provides a lightweight way to fetch rprompt data (token count,
//! cost, model) directly from the SQLite database without loading the full
//! Forge infrastructure stack.

use std::path::PathBuf;

/// Data fetched from the database for rprompt display
#[derive(Debug, Default)]
pub struct RpromptData {
    pub token_count: Option<usize>,
    pub cost: Option<f64>,
    pub model: Option<String>,
}

/// Fetches rprompt data from the SQLite database directly.
///
/// This is a fast path that bypasses the full Forge infrastructure stack.
/// Returns None on any error (DB not found, locked, invalid ID, etc.).
pub fn fetch_rprompt_data(conversation_id: &str) -> Option<RpromptData> {
    let db_path = get_database_path()?;
    let conn = rusqlite::Connection::open(&db_path).ok()?;

    let context: String = conn
        .query_row(
            "SELECT context FROM conversations WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .ok()?;

    // Use in-memory SQLite for JSON extraction
    let mem_conn = rusqlite::Connection::open_in_memory().ok()?;

    let token_count = extract_token_count(&mem_conn, &context);
    let cost = extract_cost(&mem_conn, &context);
    let model = extract_model(&mem_conn, &context);

    Some(RpromptData { token_count, cost, model })
}

fn get_database_path() -> Option<PathBuf> {
    // Use current working directory, matching how forge resolves the DB path
    // The DB is at .forge/forge.db relative to the project directory
    let cwd = std::env::current_dir().ok()?;
    Some(cwd.join(".forge").join("forge.db"))
}

fn extract_token_count(conn: &rusqlite::Connection, context: &str) -> Option<usize> {
    // Try top-level usage.total_tokens
    let result: Option<String> = conn
        .query_row(
            "SELECT json_extract(?1, '$.usage.total_tokens')",
            [context],
            |row| row.get(0),
        )
        .ok();

    if let Some(val) = result {
        return parse_token_value(&val);
    }

    // Fallback: last message's usage.total_tokens
    let messages: Option<String> = conn
        .query_row("SELECT json_extract(?1, '$.messages')", [context], |row| {
            row.get(0)
        })
        .ok()?;

    let last_message: Option<String> = conn
        .query_row("SELECT json_extract(?1, '$[-1]')", [&messages], |row| {
            row.get(0)
        })
        .ok();

    if let Some(msg) = last_message {
        let result: Option<String> = conn
            .query_row(
                "SELECT json_extract(?1, '$.usage.total_tokens')",
                [&msg],
                |row| row.get(0),
            )
            .ok();
        if let Some(val) = result {
            return parse_token_value(&val);
        }
    }

    None
}

fn parse_token_value(val: &str) -> Option<usize> {
    let val = val.trim();

    if let Some(inner) = val
        .strip_prefix("Actual(")
        .or_else(|| val.strip_prefix("Approx("))
        && let Some(num) = inner.strip_suffix(')')
    {
        return num.parse().ok();
    }

    val.parse().ok()
}

fn extract_cost(conn: &rusqlite::Connection, context: &str) -> Option<f64> {
    let result: Option<String> = conn
        .query_row(
            "SELECT json_extract(?1, '$.usage.cost')",
            [context],
            |row| row.get(0),
        )
        .ok()?;

    result.and_then(|s| s.parse().ok())
}

fn extract_model(conn: &rusqlite::Connection, context: &str) -> Option<String> {
    let result: Option<String> = conn
        .query_row(
            "SELECT json_extract(?1, '$.usage.model')",
            [context],
            |row| row.get(0),
        )
        .ok()?;

    let result = result?;
    if result.len() >= 2 && result.starts_with('"') && result.ends_with('"') {
        Some(result[1..result.len() - 1].to_string())
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mem_conn() -> Option<rusqlite::Connection> {
        rusqlite::Connection::open_in_memory().ok()
    }

    #[test]
    fn test_extract_token_count_actual() {
        let conn = create_mem_conn().unwrap();
        let context = r#"{"usage": {"total_tokens": "Actual(1500)"}}"#;
        assert_eq!(extract_token_count(&conn, context), Some(1500));
    }

    #[test]
    fn test_extract_token_count_approx() {
        let conn = create_mem_conn().unwrap();
        let context = r#"{"usage": {"total_tokens": "Approx(100)"}}"#;
        assert_eq!(extract_token_count(&conn, context), Some(100));
    }

    #[test]
    fn test_extract_token_count_raw() {
        let conn = create_mem_conn().unwrap();
        let context = r#"{"usage": {"total_tokens": "2000"}}"#;
        assert_eq!(extract_token_count(&conn, context), Some(2000));
    }

    #[test]
    fn test_extract_cost() {
        let conn = create_mem_conn().unwrap();
        let context = r#"{"usage": {"cost": "0.0123"}}"#;
        assert_eq!(extract_cost(&conn, context), Some(0.0123));
    }

    #[test]
    fn test_extract_model() {
        let conn = create_mem_conn().unwrap();
        let context = r#"{"usage": {"model": "gpt-4"}}"#;
        assert_eq!(extract_model(&conn, context), Some("gpt-4".to_string()));
    }

    #[test]
    fn test_extract_model_single_quote_edge_case() {
        let conn = create_mem_conn().unwrap();
        let context = r#"{"usage": {"model": "\""}}"#;
        assert_eq!(extract_model(&conn, context), Some("\"".to_string()));
    }
}
