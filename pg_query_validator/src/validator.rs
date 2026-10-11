//! Query validator - main validation logic

use crate::config::{ValidatorConfig, CommandType, BlockedPattern};
use regex::Regex;

pub struct QueryValidator {
    config: ValidatorConfig,
    compiled_patterns: Vec<CompiledPattern>,
    blocked_keywords_lower: Vec<String>,
}

struct CompiledPattern {
    name: String,
    regex: Regex,
    message: String,
    applies_to: Vec<CommandType>,
}

impl QueryValidator {
    pub fn new(config: ValidatorConfig) -> Self {
        let compiled_patterns = Self::compile_patterns(&config.blocked_patterns);
        let blocked_keywords_lower: Vec<String> = config
            .settings.blocked_keywords
            .iter()
            .map(|k| k.to_lowercase())
            .collect();
        Self { config, compiled_patterns, blocked_keywords_lower }
    }

    pub fn config(&self) -> &ValidatorConfig { &self.config }

    fn compile_patterns(patterns: &[BlockedPattern]) -> Vec<CompiledPattern> {
        patterns.iter().filter_map(|p| {
            let regex_str = if p.case_insensitive && !p.pattern.contains("(?i)") {
                format!("(?i){}", p.pattern)
            } else {
                p.pattern.clone()
            };
            match Regex::new(&regex_str) {
                Ok(regex) => Some(CompiledPattern {
                    name: p.name.clone(),
                    regex,
                    message: p.message.clone()
                        .unwrap_or_else(|| format!("Blocked pattern: {}", p.name)),
                    applies_to: p.applies_to.clone(),
                }),
                Err(e) => {
                    pgrx::warning!("Failed to compile pattern '{}': {}", p.name, e);
                    None
                }
            }
        }).collect()
    }

    pub fn validate_query(
        &self, query_text: &str, _query: &pgrx::pg_sys::Query,
    ) -> Result<(), String> {
        if !self.config.settings.enabled { return Ok(()); }
        let query_upper = query_text.to_uppercase();
        let command_type = self.detect_command_type(&query_upper);

        // Check query length
        if query_text.len() > self.config.settings.max_query_length {
            return Err(self.config.settings.error_messages.query_too_long.clone());
        }

        // Check blocked DDL
        if self.config.settings.block_ddl && self.is_ddl(&command_type) {
            return Err(self.config.settings.error_messages.ddl_blocked.clone());
        }

        // Check blocked TRUNCATE
        if self.config.settings.block_truncate && command_type == CommandType::Truncate {
            return Err("TRUNCATE command is blocked".into());
        }

        // Check blocked COPY
        if self.config.settings.block_copy && command_type == CommandType::Copy {
            return Err("COPY command is blocked".into());
        }

        // Check blocked patterns
        self.check_blocked_patterns(query_text, &command_type)?;

        // Check blocked keywords
        self.check_blocked_keywords(query_text)?;

        // Check WHERE clause requirement
        if self.config.settings.require_where_clause
            && (command_type == CommandType::Update || command_type == CommandType::Delete)
            && !self.has_where_clause(query_text)
        {
            return Err(self.config.settings.error_messages.where_required.clone());
        }

        // Check access control
        if let Some(ref ac) = self.config.access_control {
            self.check_access_control(query_text, &command_type, ac)?;
        }

        // Check custom rules
        for rule in &self.config.rules {
            if let Err(msg) = self.check_rule(rule, query_text, &command_type) {
                if rule.blocking { return Err(msg); }
                else { pgrx::warning!("Rule '{}' warning: {}", rule.name, msg); }
            }
        }

        if self.config.settings.log_queries {
            pgrx::log!("pg_query_validator: Query: {}", query_text);
        }
        Ok(())
    }

    pub fn validate_utility_command(&self, query_text: &str) -> Result<(), String> {
        if !self.config.settings.enabled { return Ok(()); }
        let query_upper = query_text.to_uppercase();
        let command_type = self.detect_command_type(&query_upper);

        if query_text.len() > self.config.settings.max_query_length {
            return Err(self.config.settings.error_messages.query_too_long.clone());
        }
        if self.config.settings.block_ddl && self.is_ddl(&command_type) {
            return Err(self.config.settings.error_messages.ddl_blocked.clone());
        }
        if self.config.settings.block_truncate && command_type == CommandType::Truncate {
            return Err("TRUNCATE command is blocked".into());
        }
        self.check_blocked_patterns(query_text, &command_type)?;
        self.check_blocked_keywords(query_text)?;
        Ok(())
    }

    pub fn validate_query_manual(&self, query_text: &str) -> Result<(), String> {
        if !self.config.settings.enabled { return Ok(()); }
        let query_upper = query_text.to_uppercase();
        let command_type = self.detect_command_type(&query_upper);
        if query_text.len() > self.config.settings.max_query_length {
            return Err(self.config.settings.error_messages.query_too_long.clone());
        }
        self.check_blocked_patterns(query_text, &command_type)?;
        self.check_blocked_keywords(query_text)?;
        Ok(())
    }

    fn detect_command_type(&self, query_upper: &str) -> CommandType {
        let trimmed = query_upper.trim();
        if trimmed.starts_with("SELECT") || trimmed.starts_with("WITH") { CommandType::Select }
        else if trimmed.starts_with("INSERT") { CommandType::Insert }
        else if trimmed.starts_with("UPDATE") { CommandType::Update }
        else if trimmed.starts_with("DELETE") { CommandType::Delete }
        else if trimmed.starts_with("CREATE") { CommandType::Create }
        else if trimmed.starts_with("DROP") { CommandType::Drop }
        else if trimmed.starts_with("ALTER") { CommandType::Alter }
        else if trimmed.starts_with("TRUNCATE") { CommandType::Truncate }
        else if trimmed.starts_with("COPY") { CommandType::Copy }
        else { CommandType::Select }
    }

    fn is_ddl(&self, ct: &CommandType) -> bool {
        matches!(ct, CommandType::Create | CommandType::Drop | CommandType::Alter)
    }

    fn check_blocked_patterns(&self, query: &str, ct: &CommandType) -> Result<(), String> {
        for pattern in &self.compiled_patterns {
            if !pattern.applies_to.is_empty()
                && !pattern.applies_to.contains(ct)
                && !pattern.applies_to.contains(&CommandType::All)
            { continue; }
            if pattern.regex.is_match(query) {
                return Err(pattern.message.clone());
            }
        }
        Ok(())
    }

    fn check_blocked_keywords(&self, query: &str) -> Result<(), String> {
        if self.blocked_keywords_lower.is_empty() { return Ok(()); }
        let query_lower = query.to_lowercase();
        for keyword in &self.blocked_keywords_lower {
            let pattern = format!(r"\b{}\b", regex::escape(keyword));
            if let Ok(re) = Regex::new(&pattern) {
                if re.is_match(&query_lower) {
                    return Err(format!("{}: {}",
                                       self.config.settings.error_messages.keyword_blocked, keyword));
                }
            }
        }
        Ok(())
    }

    fn has_where_clause(&self, query: &str) -> bool {
        query.to_uppercase().contains(" WHERE ")
    }

    fn check_access_control(
        &self, query: &str, ct: &CommandType, ac: &crate::config::AccessControl,
    ) -> Result<(), String> {
        let qu = query.to_uppercase();
        for table in &ac.blocked_tables {
            if qu.contains(&format!(" {} ", table.to_uppercase())) {
                return Err(format!("{}: table '{}'",
                                   self.config.settings.error_messages.access_denied, table));
            }
        }
        for schema in &ac.blocked_schemas {
            if qu.contains(&format!("{}.", schema.to_uppercase())) {
                return Err(format!("{}: schema '{}'",
                                   self.config.settings.error_messages.access_denied, schema));
            }
        }
        if matches!(ct, CommandType::Insert | CommandType::Update | CommandType::Delete) {
            for table in &ac.read_only_tables {
                if qu.contains(&format!(" {} ", table.to_uppercase())) {
                    return Err(format!("{}: table '{}' is read-only",
                                       self.config.settings.error_messages.access_denied, table));
                }
            }
        }
        Ok(())
    }

    fn check_rule(
        &self, rule: &crate::config::ValidationRule, query: &str, ct: &CommandType,
    ) -> Result<(), String> {
        use crate::config::RuleType;
        if !rule.applies_to.is_empty()
            && !rule.applies_to.contains(ct)
            && !rule.applies_to.contains(&CommandType::All)
        { return Ok(()); }

        let msg = rule.message.clone()
            .unwrap_or_else(|| format!("Rule '{}' violated", rule.name));

        match rule.rule_type {
            RuleType::RegexBlock => {
                if let Some(pattern) = rule.config.get("pattern").and_then(|v| v.as_str()) {
                    if let Ok(re) = Regex::new(pattern) {
                        if re.is_match(query) { return Err(msg); }
                    }
                }
            }
            RuleType::RequireClause => {
                if let Some(clause) = rule.config.get("clause").and_then(|v| v.as_str()) {
                    if !query.to_uppercase().contains(&clause.to_uppercase()) {
                        return Err(msg);
                    }
                }
            }
            RuleType::ComplexityLimit => {
                if let Some(max_joins) = rule.config.get("max_joins").and_then(|v| v.as_u64()) {
                    let count = query.to_uppercase().matches(" JOIN ").count();
                    if count > max_joins as usize {
                        return Err(format!("{}: too many JOINs", msg));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}