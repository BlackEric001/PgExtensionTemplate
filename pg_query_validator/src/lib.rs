//! pg_query_validator - PostgreSQL extension for automatic query validation
//!
//! This extension intercepts all incoming SQL queries using PostgreSQL hooks
//! and validates them against configurable rules loaded from a JSON file.
//! No explicit function calls are needed from the user side.

use pgrx::prelude::*;
use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use pgrx::GucSetting;
use std::ffi::CString;

mod config;
mod rules;
mod validator;
mod utils;

use config::ValidatorConfig;
use validator::QueryValidator;

// Global state for the validator
static mut VALIDATOR: Option<QueryValidator> = None;
static INITIALIZED: AtomicBool = AtomicBool::new(false);

// Store original hooks for chaining
static mut PREV_POST_PARSE_ANALYZE_HOOK:
pgrx::pg_sys::post_parse_analyze_hook_type = None;

static mut PREV_PROCESS_UTILITY_HOOK:
pgrx::pg_sys::ProcessUtility_hook_type = None;

// GUC variable for config file path
static CONFIG_PATH: GucSetting<Option<CString>> = GucSetting::<Option<CString>>::new(None);

pgrx::pg_module_magic!();

/// Extension initialization - called when the shared library is loaded
#[allow(non_snake_case)]
#[pg_guard]
pub extern "C-unwind" fn _PG_init() {
    // Define GUC for config file path
        pgrx::GucRegistry::define_string_guc(
            c"pg_query_validator.config_path",
            c"Path to the JSON configuration file.",
            c"This file contains rules for validating incoming SQL queries.",
            &CONFIG_PATH,
            pgrx::GucContext::Suset,
            pgrx::GucFlags::SUPERUSER_ONLY,
        );

    // Install hooks
    unsafe {
        install_hooks();
    }

    // Load configuration
    load_config();

    INITIALIZED.store(true, Ordering::SeqCst);
    pgrx::log!("pg_query_validator: Extension initialized successfully");
}

/// Install PostgreSQL hooks for query interception
unsafe fn install_hooks() {
    PREV_POST_PARSE_ANALYZE_HOOK =
        pgrx::pg_sys::post_parse_analyze_hook;

    pgrx::pg_sys::post_parse_analyze_hook =
        Some(post_parse_analyze_hook);

    PREV_PROCESS_UTILITY_HOOK =
        pgrx::pg_sys::ProcessUtility_hook;

    pgrx::pg_sys::ProcessUtility_hook =
        Some(process_utility_hook);
}

/// Load configuration from JSON file
fn load_config() {
    let config_path = get_config_path();

    match ValidatorConfig::load_from_file(&config_path) {
        Ok(config) => {
            let validator = QueryValidator::new(config);
            unsafe { VALIDATOR = Some(validator); }
            pgrx::log!("pg_query_validator: Config loaded from {}", config_path);
        }
        Err(e) => {
            pgrx::warning!("pg_query_validator: Failed to load config: {}. Using defaults.", e);
            let config = ValidatorConfig::default();
            let validator = QueryValidator::new(config);
            unsafe { VALIDATOR = Some(validator); }
        }
    }
}

fn get_config_path() -> String {
    if let Some(value) = CONFIG_PATH.get() {
        if let Ok(path) = value.into_string() {
            if !path.is_empty() {
                return path;
            }
        }
    }

    utils::default_config_path()
}

/// Hook: Called after a query is parsed and analyzed
#[pg_guard]
unsafe extern "C-unwind" fn post_parse_analyze_hook(
    pstate: *mut pgrx::pg_sys::ParseState,
    query: *mut pgrx::pg_sys::Query,
    query_env: *mut pgrx::pg_sys::JumbleState,
) {
    // Call previous hook first (chain)
    if let Some(prev_hook) = PREV_POST_PARSE_ANALYZE_HOOK {
        prev_hook(pstate, query, query_env);
    }

    // Validate the query
    if let Some(ref validator) = VALIDATOR {
        if !query.is_null() {
            // Skip system sql queries
            if is_system_query(query, pstate) {
                return;
            }

            if let Some(query_text) = get_query_text(pstate) {
                pgrx::log!("pg_query_validator: Validating user query: {}", query_text);
                if let Err(error_msg) = validator.validate_query(&query_text) {
                    pgrx::error!("Query validation failed: {}", error_msg);
                }
            }
        }
    }
}

/// Hook: Called for utility commands (CREATE, DROP, ALTER, etc.)
#[pg_guard]
unsafe extern "C-unwind" fn process_utility_hook(
    pstmt: *mut pgrx::pg_sys::PlannedStmt,
    query_string: *const std::os::raw::c_char,
    read_only_tree: bool,
    context: pgrx::pg_sys::ProcessUtilityContext::Type,
    params: pgrx::pg_sys::ParamListInfo,
    query_env: *mut pgrx::pg_sys::QueryEnvironment,
    dest: *mut pgrx::pg_sys::DestReceiver,
    query_completion: *mut pgrx::pg_sys::QueryCompletion,
) {
    // Validate utility commands
    if let Some(ref validator) = VALIDATOR {
        if !query_string.is_null() {
            let c_str = CStr::from_ptr(query_string);
            if let Ok(query_text) = c_str.to_str() {
                if let Err(error_msg) = validator.validate_utility_command(query_text) {
                    pgrx::error!("Query validation failed: {}", error_msg);
                }
            }
        }
    }

    // Call previous hook or standard handler
    if let Some(prev_hook) = PREV_PROCESS_UTILITY_HOOK {
        prev_hook(pstmt, query_string, read_only_tree, context,
                  params, query_env, dest, query_completion);
    } else {
        pgrx::pg_sys::standard_ProcessUtility(
            pstmt, query_string, read_only_tree, context,
            params, query_env, dest, query_completion);
    }
}

unsafe fn get_query_text(pstate: *mut pgrx::pg_sys::ParseState) -> Option<String> {
    if pstate.is_null() { return None; }
    let ps = &*pstate;
    if ps.p_sourcetext.is_null() { return None; }
    let c_str = CStr::from_ptr(ps.p_sourcetext);
    c_str.to_str().ok().map(|s| s.to_string())
}

/// SQL function to reload configuration without restart
#[pg_extern(security_definer)]
fn reload_config() -> String {
    load_config();
    "Configuration reloaded successfully".to_string()
}

/// SQL function to get current configuration
#[pg_extern(security_definer)]
fn get_config() -> String {
    unsafe {
        if let Some(ref validator) = VALIDATOR {
            serde_json::to_string_pretty(validator.config())
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        } else {
            "{\"error\": \"Validator not initialized\"}".to_string()
        }
    }
}

/// SQL function to validate a query string manually (for testing)
#[pg_extern(security_definer)]
fn validate_query(query_text: &str) -> String {
    unsafe {
        if let Some(ref validator) = VALIDATOR {
            match validator.validate_query_manual(query_text) {
                Ok(()) => "{\"valid\": true}".to_string(),
                Err(msg) => format!("{{\"valid\": false, \"reason\": \"{}\"}}", msg),
            }
        } else {
            "{\"error\": \"Validator not initialized\"}".to_string()
        }
    }
}

/// Determines whether the request is system-generated (not user-generated)
unsafe fn is_system_query(query: *mut pgrx::pg_sys::Query, pstate: *mut pgrx::pg_sys::ParseState) -> bool {
    if query.is_null() {
        return false;
    }

    let q = &*query;

    // Checking the source of the sql query
    // QSRC_ORIGINAL = query from the user
    // Other values = system/internal queries
    if q.querySource != pgrx::pg_sys::QuerySource::QSRC_ORIGINAL {
        pgrx::log!("pg_query_validator: Skipping system query (source: {:?})", q.querySource);
        return true;
    }

    // Additionally check application_name
    let app_name = pgrx::pg_sys::application_name;
    if !app_name.is_null() {
        if let Ok(c_str) = CStr::from_ptr(app_name).to_str() {
            let app = c_str.to_lowercase();
            let system_apps = [
                "pg_dump", "pg_restore", "vacuum", "autovacuum",
                "wal sender", "walsender", "walwriter", "bgworker",
                "archiver", "stats collector"
            ];

            for sys_app in &system_apps {
                if app.contains(sys_app) {
                    pgrx::log!("pg_query_validator: Skipping query from system app: {}", c_str);
                    return true;
                }
            }
        }
    }

    // Checking if the query accesses system tables
    if !pstate.is_null() {
        let ps = &*pstate;
        if !ps.p_sourcetext.is_null() {
            if let Ok(source) = CStr::from_ptr(ps.p_sourcetext).to_str() {
                let source_upper = source.to_uppercase();

                // Пропускаем запросы к системным каталогам
                if source_upper.contains("PG_CATALOG.")
                    || source_upper.contains("INFORMATION_SCHEMA.")
                    || (source_upper.contains("PG_") && !source_upper.contains(" FROM PG_")) {
                    pgrx::log!("pg_query_validator: Skipping query to system catalog");
                    return true;
                }
            }
        }
    }

    false
}