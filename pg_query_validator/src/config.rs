//! Configuration module - handles loading and parsing of JSON configuration file

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    #[serde(default)]
    pub settings: GlobalSettings,
    #[serde(default)]
    pub rules: Vec<ValidationRule>,
    #[serde(default)]
    pub blocked_patterns: Vec<BlockedPattern>,
    #[serde(default)]
    pub access_control: Option<AccessControl>,
    #[serde(default)]
    pub rate_limiting: Option<RateLimiting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub log_queries: bool,
    #[serde(default = "default_true")]
    pub log_blocked: bool,
    #[serde(default = "default_max_query_length")]
    pub max_query_length: usize,
    #[serde(default = "default_max_statements")]
    pub max_statements: usize,
    #[serde(default)]
    pub block_ddl: bool,
    #[serde(default)]
    pub block_truncate: bool,
    #[serde(default)]
    pub block_copy: bool,
    #[serde(default = "default_true")]
    pub require_where_clause: bool,
    #[serde(default)]
    pub require_limit: bool,
    #[serde(default = "default_max_tables")]
    pub max_tables_in_query: usize,
    #[serde(default)]
    pub blocked_keywords: Vec<String>,
    #[serde(default)]
    pub error_messages: ErrorMessages,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorMessages {
    #[serde(default = "default_query_too_long_msg")]
    pub query_too_long: String,
    #[serde(default = "default_blocked_pattern_msg")]
    pub blocked_pattern: String,
    #[serde(default = "default_where_required_msg")]
    pub where_required: String,
    #[serde(default = "default_ddl_blocked_msg")]
    pub ddl_blocked: String,
    #[serde(default = "default_access_denied_msg")]
    pub access_denied: String,
    #[serde(default = "default_keyword_blocked_msg")]
    pub keyword_blocked: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub rule_type: RuleType,
    #[serde(default = "default_true")]
    pub blocking: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub applies_to: Vec<CommandType>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    RegexBlock,
    RequireClause,
    ComplexityLimit,
    TableAccess,
    TimeBased,
    IpBased,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandType {
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Alter,
    Truncate,
    Copy,
    Grant,
    Revoke,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedPattern {
    pub name: String,
    pub pattern: String,
    #[serde(default = "default_true")]
    pub case_insensitive: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub applies_to: Vec<CommandType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControl {
    #[serde(default)]
    pub read_only_tables: Vec<String>,
    #[serde(default)]
    pub blocked_tables: Vec<String>,
    #[serde(default)]
    pub blocked_schemas: Vec<String>,
    #[serde(default)]
    pub allowed_schemas: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiting {
    #[serde(default = "default_max_qps")]
    pub max_queries_per_second: u32,
    #[serde(default = "default_max_qpm")]
    pub max_queries_per_minute: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
fn default_max_query_length() -> usize {
    1_000_000
}
fn default_max_statements() -> usize {
    10
}
fn default_max_tables() -> usize {
    50
}
fn default_max_qps() -> u32 {
    100
}
fn default_max_qpm() -> u32 {
    1000
}
fn default_query_too_long_msg() -> String {
    "Query exceeds maximum allowed length".into()
}
fn default_blocked_pattern_msg() -> String {
    "Query matches a blocked pattern".into()
}
fn default_where_required_msg() -> String {
    "WHERE clause is required".into()
}
fn default_ddl_blocked_msg() -> String {
    "DDL operations are not allowed".into()
}
fn default_access_denied_msg() -> String {
    "Access denied".into()
}
fn default_keyword_blocked_msg() -> String {
    "Query contains a blocked keyword".into()
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            log_queries: false,
            log_blocked: true,
            max_query_length: 1_000_000,
            max_statements: 10,
            block_ddl: false,
            block_truncate: false,
            block_copy: false,
            require_where_clause: true,
            require_limit: false,
            max_tables_in_query: 50,
            blocked_keywords: Vec::new(),
            error_messages: ErrorMessages::default(),
        }
    }
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            settings: GlobalSettings::default(),
            rules: Vec::new(),
            blocked_patterns: vec![],
            access_control: None,
            rate_limiting: None,
        }
    }
}

impl ValidatorConfig {
    pub fn load_from_file(path: &str) -> Result<Self, String> {
        let path = Path::new(path);
        if !path.exists() {
            return Err(format!("Config file not found: {}", path.display()));
        }
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config: {}", e))?;
        Self::from_json(&content)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse config JSON: {}", e))
    }

    pub fn validate(&self) -> Result<(), String> {
        for pattern in &self.blocked_patterns {
            if regex::Regex::new(&pattern.pattern).is_err() {
                return Err(format!("Invalid regex in pattern '{}'", pattern.name));
            }
        }
        Ok(())
    }
}
