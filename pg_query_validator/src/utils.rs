use std::path::PathBuf;

pub fn default_config_path() -> String {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Windows-specific locations
    #[cfg(windows)]
    {
        if let Some(program_data) = std::env::var_os("PROGRAMDATA") {
            candidates.push(
                PathBuf::from(program_data)
                    .join("pg_query_validator")
                    .join("pg_query_validator.json"),
            );
        }

        if let Some(pgdata) = std::env::var_os("PGDATA") {
            candidates.push(
                PathBuf::from(pgdata)
                    .join("pg_query_validator.json"),
            );
        }
    }

    // Linux and other Unix-like systems
    #[cfg(unix)]
    {
        candidates.push(
            PathBuf::from("/etc/postgresql/pg_query_validator.json"),
        );

        candidates.push(
            PathBuf::from("/var/lib/postgresql/pg_query_validator.json"),
        );
    }

    for path in candidates {
        if path.is_file() {
            return path.to_string_lossy().into_owned();
        }
    }

    // Platform-specific fallback, used when no file exists.
    #[cfg(windows)]
    {
        PathBuf::from(r"C:\ProgramData\pg_query_validator\pg_query_validator.json")
            .to_string_lossy()
            .into_owned()
    }

    #[cfg(unix)]
    {
        "/etc/postgresql/pg_query_validator.json".to_string()
    }

    #[cfg(not(any(windows, unix)))]
    {
        "pg_query_validator.json".to_string()
    }
}
