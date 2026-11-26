//! M5 (Appendix L 7.4, M5-S38): the `migrate` command.
//!
//! `lykaon migrate` runs the files in the `migrations` folder in file-name
//! order, each once, against the database named by `settings.lykaon`.
//! Applied files are recorded in `lykaon_migrations`. A failed file rolls back
//! and stops the command. `lykaon migrate status` lists applied and pending
//! files. There is no down.
//!
//! The migration files are SQL scripts in this increment (`.sql`). Appendix
//! L describes Lykaon migration sentences; those parse to the same SQL for
//! SQLite and PostgreSQL and can replace the file reader later. Logged.

use std::path::{Path, PathBuf};

use crate::db;

/// The database named by settings.lykaon: (name, driver).
fn database_from_settings(root: &Path) -> Result<(String, String), String> {
    let settings = root.join("settings.lykaon");
    let text = match std::fs::read_to_string(&settings) {
        Ok(t) => t,
        // compatibility with the old settings.tern spelling (the book's)
        Err(_) => std::fs::read_to_string(root.join("settings.tern")).map_err(|_| {
            "settings.lykaon is missing (it must name the database).".to_string()
        })?,
    };
    let mut name = String::new();
    let mut driver = String::new();
    for line in text.lines() {
        let lower = line.trim().to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("database is ") {
            let rest = rest.trim().trim_end_matches('.');
            if rest.starts_with("sqlite") || rest.starts_with("postgres") {
                driver = rest.to_string();
            } else if name.is_empty() {
                name = rest.to_string();
            }
        } else if let Some(rest) = lower.strip_prefix("database uses ") {
            driver = rest.trim().trim_end_matches('.').to_string();
        }
    }
    if name.is_empty() {
        return Err("settings.lykaon has no `Database is NAME.` sentence.".into());
    }
    Ok((name, driver))
}

fn open_database(root: &Path, name: &str, driver: &str) -> Result<(), String> {
    if driver.is_empty() || driver.starts_with("sqlite") {
        // `sqlite file <name>` or just a name (the file is NAME.sqlite)
        let path = if let Some(rest) = driver.strip_prefix("sqlite") {
            rest.trim()
                .strip_prefix("file ")
                .unwrap_or(rest.trim())
                .to_string()
        } else {
            format!("{name}.sqlite")
        };
        db::open_sqlite(name, &root.join(path).to_string_lossy())
    } else if driver.starts_with("postgres") {
        // `postgres at <host> <port> as <user> ...` is not parsed here yet;
        // the address words of M4-S58 can name a local server.
        let rest = driver.trim();
        let rest = rest.strip_prefix("postgres").unwrap_or(rest).trim();
        let rest = rest.strip_prefix("at ").unwrap_or(rest);
        let mut it = rest.split_whitespace();
        let host = it.next().unwrap_or("localhost");
        let port: u16 = it.next().and_then(|p| p.parse().ok()).unwrap_or(5432);
        let user = std::env::var("Lykaon_PGUSER")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "postgres".to_string());
        let password = std::env::var("Lykaon_PGPASSWORD").unwrap_or_default();
        let database = name.to_string();
        db::open_postgres(name, host, port, &user, &password, &database)
    } else {
        Err(format!("the database driver {driver:?} is not one of sqlite or postgres."))
    }
}

fn migration_files(root: &Path) -> Vec<PathBuf> {
    let dir = root.join("migrations");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("sql"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

/// Run (or, with `status`, only list) the migrations. Returns the text to
/// print.
pub fn run(root: &Path, status_only: bool) -> Result<String, String> {
    let (name, driver) = database_from_settings(root)?;
    open_database(root, &name, &driver)?;
    db::execute_sql(
        &name,
        "CREATE TABLE IF NOT EXISTS lykaon_migrations (name TEXT PRIMARY KEY)",
    )
    .map_err(|e| format!("T0523: {e}"))?;
    let files = migration_files(root);
    let applied: Vec<String> = db::query_strings(&name, "SELECT name FROM lykaon_migrations")?
        .into_iter()
        .map(|r| r.into_iter().next().unwrap_or_default())
        .collect();
    let mut out = String::new();
    for path in &files {
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let is_applied = applied.iter().any(|a| a == &file);
        if status_only {
            out.push_str(if is_applied { "applied " } else { "pending " });
            out.push_str(&file);
            out.push('\n');
            continue;
        }
        if is_applied {
            continue;
        }
        let sql = std::fs::read_to_string(path)
            .map_err(|e| format!("T0523: {} cannot be read: {e}", path.display()))?;
        db::execute_sql(&name, "BEGIN")
            .map_err(|e| format!("T0523: {e}"))?;
        let result = db::execute_sql(&name, &sql).and_then(|_| {
            db::execute_sql(
                &name,
                &format!(
                    "INSERT INTO lykaon_migrations (name) VALUES ('{}')",
                    file.replace('\'', "''")
                ),
            )
        });
        match result {
            Ok(()) => {
                db::execute_sql(&name, "COMMIT").map_err(|e| format!("T0523: {e}"))?;
                out.push_str("applied ");
                out.push_str(&file);
                out.push('\n');
            }
            Err(e) => {
                db::rollback_all();
                return Err(format!(
                    "T0523: migration {} failed and was rolled back: {e}",
                    path.display()
                ));
            }
        }
    }
    if !status_only && files.is_empty() {
        out.push_str("no migration files in migrations/.\n");
    }
    Ok(out)
}