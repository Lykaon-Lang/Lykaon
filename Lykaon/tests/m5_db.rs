//! M5 (Appendix L 7.4) conformance: SQLite open/add/load/remove/set,
//! transactions (commit and rollback), nested-transaction refusal, the
//! `migrate` command and the PostgreSQL connection failure.
//!
//! Run: cargo test --test m5_db -- --nocapture

use lykaon::run_source;

fn run(src: &str) -> (String, i32) {
    run_source(src, "program.lykaon", "")
}

fn out(src: &str) -> String {
    let (out, code) = run(src);
    assert_eq!(code, 0, "the program failed: {out}");
    out.trim_end().to_string()
}

fn fresh_db(name: &str, file: &str) {
    lykaon::db::close(name);
    let _ = std::fs::remove_file(file);
    lykaon::db::open_sqlite(name, file).expect("open the test database");
    lykaon::db::execute_sql(
        name,
        "CREATE TABLE stock (name TEXT, count INTEGER, price INTEGER)",
    )
    .expect("create the table");
}

fn rows(name: &str) -> Vec<Vec<String>> {
    lykaon::db::query_strings(name, "SELECT name, count, price FROM stock ORDER BY name")
        .expect("read the table")
}

const DECL: &str = "A stockitem has name as text and count as whole number and price as whole number.\n";

#[test]
fn m5_t24_sqlite_add_load_and_print() {
    let file = "m5db1";
    fresh_db("m5db1", file);
    let text = out(&format!(
        "{DECL}Open database shop using sqlite file {file}.\nAdd a stockitem with name apples and count 5 and price 3 to table stock of shop.\nLoad stock from database shop.\nSay rows."
    ));
    assert_eq!(text, "rows 1");
    assert_eq!(rows("m5db1"), vec![vec!["apples", "5", "3"]]);
    lykaon::db::close("m5db1");
    let _ = std::fs::remove_file(file);
}

#[test]
fn m5_remove_and_set_rows() {
    let file = "m5db2";
    fresh_db("m5db2", file);
    lykaon::db::execute_sql(
        "m5db2",
        "INSERT INTO stock VALUES ('apples',5,3), ('pears',7,4), ('plums',2,9)",
    )
    .unwrap();
    let src = format!(
        "{DECL}Open database shop using sqlite file {file}.\nRemove rows where name is pears from table stock of shop.\nSet count to 11 in rows of stock of shop where name is apples."
    );
    let (_, code) = run(&src);
    assert_eq!(code, 0);
    assert_eq!(
        rows("m5db2"),
        vec![
            vec!["apples", "11", "3"],
            vec!["plums", "2", "9"]
        ]
    );
    lykaon::db::close("m5db2");
    let _ = std::fs::remove_file(file);
}

#[test]
fn m5_transaction_commits() {
    let file = "m5db3";
    fresh_db("m5db3", file);
    let src = format!(
        "{DECL}Open database shop using sqlite file {file}.\nDo these all or none.\n    Add a stockitem with name plums and count 2 and price 7 to table stock of shop."
    );
    let (_, code) = run(&src);
    assert_eq!(code, 0);
    let names: Vec<String> = rows("m5db3").into_iter().map(|r| r[0].clone()).collect();
    assert_eq!(names, vec!["plums"]);
    lykaon::db::close("m5db3");
    let _ = std::fs::remove_file(file);
}

#[test]
fn m5_transaction_rolls_back_on_failure() {
    let file = "m5db4";
    fresh_db("m5db4", file);
    lykaon::db::execute_sql("m5db4", "INSERT INTO stock VALUES ('plums',2,7)").unwrap();
    let src = format!(
        "{DECL}Open database shop using sqlite file {file}.\nDo these all or none.\n    Add a stockitem with name bad and count 1 and price 1 to table stock of shop.\n    Exit with 1."
    );
    let (_, code) = run(&src);
    assert_eq!(code, 1, "Exit with 1 ends the run");
    let names: Vec<String> = rows("m5db4").into_iter().map(|r| r[0].clone()).collect();
    assert_eq!(names, vec!["plums"], "the failed transaction rolled back");
    lykaon::db::close("m5db4");
    let _ = std::fs::remove_file(file);
}

#[test]
fn m5_nested_transaction_is_t0188() {
    let file = "m5db5";
    fresh_db("m5db5", file);
    let src = format!(
        "{DECL}Open database shop using sqlite file {file}.\nDo these all or none.\n    Do these all or none.\n        Add a stockitem with name plums and count 2 and price 7 to table stock of shop."
    );
    let (text, code) = run(&src);
    assert_eq!(code, 1, "nested transactions fail: {text}");
    assert!(text.contains("T0188"), "expected T0188, got: {text}");
    lykaon::db::close("m5db5");
    let _ = std::fs::remove_file(file);
}

#[test]
fn m5_open_failure_is_t0522() {
    // A directory cannot be opened as a SQLite database file.
    let dir = "m5dbdir";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let (text, code) = run(&format!(
        "{DECL}Open database shop using sqlite file {dir}."
    ));
    assert_eq!(code, 1, "open must fail: {text}");
    assert!(text.contains("T0522"), "expected T0522, got: {text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn m5_migrate_runs_each_file_once() {
    let dir = std::env::temp_dir().join(format!("lykaon_mig_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::write(
        dir.join("settings.lykaon"),
        "Database is shop.\nDatabase uses sqlite file shopdata.\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("migrations").join("001.sql"),
        "CREATE TABLE stock (name TEXT, count INTEGER);",
    )
    .unwrap();
    std::fs::write(
        dir.join("migrations").join("002.sql"),
        "ALTER TABLE stock ADD COLUMN price INTEGER;",
    )
    .unwrap();
    let first = lykaon::migrate::run(&dir, false).expect("first migrate");
    assert!(first.contains("applied 001.sql"));
    assert!(first.contains("applied 002.sql"));
    let second = lykaon::migrate::run(&dir, false).expect("second migrate");
    assert_eq!(second, "", "the second run applies nothing");
    let status = lykaon::migrate::run(&dir, true).expect("status");
    assert!(status.contains("applied 001.sql"));
    assert!(status.contains("applied 002.sql"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m5_bad_migration_is_t0523() {
    let dir = std::env::temp_dir().join(format!("lykaon_migbad_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("migrations")).unwrap();
    std::fs::write(
        dir.join("settings.lykaon"),
        "Database is shop.\nDatabase uses sqlite file shopdata.\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("migrations").join("001.sql"),
        "CREATE TABLE stock (name TEXT);",
    )
    .unwrap();
    std::fs::write(
        dir.join("migrations").join("002.sql"),
        "CREATE TABLE",
    )
    .unwrap();
    let err = lykaon::migrate::run(&dir, false).expect_err("the bad file fails");
    assert!(err.contains("T0523"), "expected T0523, got: {err}");
    // the first file stays applied, the second is not recorded
    let status = lykaon::migrate::run(&dir, true).expect("status");
    assert!(status.contains("applied 001.sql"));
    assert!(status.contains("pending 002.sql"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m5_postgres_without_a_server_is_t0522() {
    // Nothing listens on this port in the test environment.
    let (text, code) = run(&format!(
        "{DECL}Open database pg using postgres at localhost 1."
    ));
    assert_eq!(code, 1, "the connection must fail: {text}");
    assert!(text.contains("T0522"), "expected T0522, got: {text}");
}

/// Real-server round trip. Skipped unless LYKAON_PG_TEST=1 and a PostgreSQL
/// server is reachable. The defaults match the Docker command used for the
/// M5 verification:
///   docker run -d --name lykaon-pg -e POSTGRES_USER=lykaon \
///     -e POSTGRES_PASSWORD=lykaon -e POSTGRES_DB=lykaon \
///     -e POSTGRES_HOST_AUTH_METHOD=md5 -p 5432:5432 postgres:16
#[test]
fn m5_postgres_real_server_round_trip() {
    if std::env::var("LYKAON_PG_TEST").is_err() {
        return;
    }
    use lykaon_core::sem::Value;
    let host = std::env::var("LYKAON_PG_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("LYKAON_PG_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5432);
    let user = std::env::var("LYKAON_PG_USER").unwrap_or_else(|_| "lykaon".to_string());
    let password = std::env::var("LYKAON_PG_PASSWORD").unwrap_or_else(|_| "lykaon".to_string());
    let dbname = std::env::var("LYKAON_PG_DB").unwrap_or_else(|_| "lykaon".to_string());
    let host_words = host
        .split('.')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" dot ");
    // The interpreter reads the credentials from the environment.
    unsafe {
        std::env::set_var("Lykaon_PGUSER", &user);
        std::env::set_var("Lykaon_PGPASSWORD", &password);
    }

    lykaon::db::close("pgtest");
    lykaon::db::open_postgres("pgtest", &host, port, &user, &password, &dbname)
        .expect("open the real PostgreSQL server");
    lykaon::db::execute_sql(
        "pgtest",
        "DROP TABLE IF EXISTS lykaonpgtest; CREATE TABLE lykaonpgtest (n int);",
    )
    .expect("create the test table");

    let src = format!(
        "A thing has n as whole number.\n\
         Open database pg using postgres at {host_words} on port {port} path {dbname}.\n\
         Do these all or none.\n\
             Add a thing with n 7 to table lykaonpgtest of pg.\n\
             Add a thing with n 9 to table lykaonpgtest of pg.\n\
             Remove rows where n is 7 from table lykaonpgtest of pg.\n\
             Set n to 11 in rows of lykaonpgtest of pg where n is 9.\n\
         Load lykaonpgtest from database pg.\n\
         Say rows.\n"
    );
    let (out, code) = run_source(&src, "pg_test.lykaon", "");
    assert_eq!(code, 0, "the PostgreSQL program failed: {out}");
    assert_eq!(out.trim(), "rows 1");

    let t = lykaon::db::read_table("pgtest", "lykaonpgtest").expect("read the table");
    assert_eq!(t.rows.len(), 1, "one row must survive: {:?}", t.rows);
    assert!(
        matches!(t.rows[0][0], Value::Whole(11)),
        "the remaining row must be 11: {:?}",
        t.rows
    );

    lykaon::db::close("pgtest");
    lykaon::db::close("pg");
    let _ = std::fs::remove_file("pg_test.lykaon");
}