//! M5 (Appendix L 7.4): database drivers and open connections.
//!
//! SQLite is bundled (rusqlite) so the one-binary promise holds. The
//! PostgreSQL driver is a small wire-protocol client written here (no
//! external crate): startup, cleartext/MD5 authentication, simple queries,
//! `Load` and `Add`, and transactions. SCRAM-SHA-256 is not implemented yet
//! (logged); `Remove`/`Set` still use the SQLite rowid path.
//!
//! A connection is named by the program (`Open database shop using ...`)
//! and the value `shop` holds that name. The driver state lives in this
//! module's thread-local registry, never in `Value` (a connection is not a
//! copyable value and never crosses the bridge).

use std::cell::RefCell;
use std::collections::HashMap;

use crate::data::Table;
use crate::sem::{Kind, Value};

pub mod pg {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    /// One open PostgreSQL session. Only simple queries are supported.
    pub struct PgConn {
        stream: TcpStream,
    }

    fn md5_hex(data: &[u8]) -> String {
        format!("{:x}", md5::compute(data))
    }

    fn send(stream: &mut TcpStream, ty: u8, body: &[u8]) -> std::io::Result<()> {
        let len = (body.len() + 4) as i32;
        stream.write_all(&[ty])?;
        stream.write_all(&len.to_be_bytes())?;
        stream.write_all(body)?;
        Ok(())
    }

    fn read_exact(stream: &mut TcpStream, n: usize) -> std::io::Result<Vec<u8>> {
        let mut v = vec![0u8; n];
        stream.read_exact(&mut v)?;
        Ok(v)
    }

    fn read_msg(stream: &mut TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
        let ty = {
            let mut b = [0u8; 1];
            stream.read_exact(&mut b)?;
            b[0]
        };
        let len = i32::from_be_bytes(read_exact(stream, 4)?.try_into().unwrap()) as usize;
        let body = read_exact(stream, len.saturating_sub(4))?;
        Ok((ty, body))
    }

    impl PgConn {
        pub fn connect(
            host: &str,
            port: u16,
            user: &str,
            password: &str,
            database: &str,
        ) -> Result<Self, String> {
            let mut stream = TcpStream::connect((host, port))
                .map_err(|e| format!("the server at {host}:{port} is not reachable: {e}"))?;
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));
            let mut body = Vec::new();
            body.extend_from_slice(&196608i32.to_be_bytes());
            for (k, v) in [
                ("user", user),
                ("database", database),
                ("client_encoding", "UTF8"),
            ] {
                body.extend_from_slice(k.as_bytes());
                body.push(0);
                body.extend_from_slice(v.as_bytes());
                body.push(0);
            }
            body.push(0);
            let len = (body.len() + 4) as i32;
            stream
                .write_all(&len.to_be_bytes())
                .and_then(|_| stream.write_all(&body))
                .map_err(|e| e.to_string())?;
            loop {
                let (ty, msg) = read_msg(&mut stream).map_err(|e| e.to_string())?;
                match ty {
                    b'R' => {
                        if msg.len() < 4 {
                            return Err("the server sent a bad authentication message.".into());
                        }
                        let code = i32::from_be_bytes(msg[0..4].try_into().unwrap());
                        match code {
                            0 => {}
                            3 => {
                                let mut b = password.as_bytes().to_vec();
                                b.push(0);
                                send(&mut stream, b'p', &b).map_err(|e| e.to_string())?;
                            }
                            5 => {
                                if msg.len() < 8 {
                                    return Err("the server sent a bad MD5 challenge.".into());
                                }
                                let salt = &msg[4..8];
                                let inner = md5_hex(format!("{password}{user}").as_bytes());
                                let mut outer = inner.into_bytes();
                                outer.extend_from_slice(salt);
                                let outer = md5_hex(&outer);
                                let mut b = format!("md5{outer}").into_bytes();
                                b.push(0);
                                send(&mut stream, b'p', &b).map_err(|e| e.to_string())?;
                            }
                            10 => {
                                return Err(
                                    "the PostgreSQL server asks for SCRAM-SHA-256, which this driver does not implement yet."
                                        .into(),
                                );
                            }
                            other => {
                                return Err(format!(
                                    "the PostgreSQL server asked for an authentication method this driver does not support ({other})."
                                ));
                            }
                        }
                    }
                    b'E' => return Err(pg_error(&msg)),
                    b'Z' => return Ok(PgConn { stream }),
                    _ => {}
                }
            }
        }

        /// Run one simple query and return the column names and rows as text
        /// (NULL = empty). This is the M5-S33/S35 path for PostgreSQL.
        pub fn query(&mut self, sql: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
            let mut body = sql.as_bytes().to_vec();
            body.push(0);
            send(&mut self.stream, b'Q', &body).map_err(|e| e.to_string())?;
            let mut names: Vec<String> = Vec::new();
            let mut rows = Vec::new();
            loop {
                let (ty, msg) = read_msg(&mut self.stream).map_err(|e| e.to_string())?;
                match ty {
                    b'T' => {
                        // RowDescription: int16 count, then per field a name
                        // cstring and 18 bytes of metadata.
                        if msg.len() < 2 {
                            return Err("the server sent a bad row description.".into());
                        }
                        let n = i16::from_be_bytes(msg[0..2].try_into().unwrap()) as usize;
                        let mut p = 2usize;
                        names.clear();
                        for _ in 0..n {
                            let end = msg[p..]
                                .iter()
                                .position(|b| *b == 0)
                                .ok_or("the server sent a bad column name.")?;
                            names.push(String::from_utf8_lossy(&msg[p..p + end]).to_string());
                            p += end + 1 + 18;
                        }
                    }
                    b'D' => {
                        if msg.len() < 2 {
                            return Err("the server sent a bad data row.".into());
                        }
                        let n = i16::from_be_bytes(msg[0..2].try_into().unwrap()) as usize;
                        let mut p = 2usize;
                        let mut row = Vec::with_capacity(n);
                        for _ in 0..n {
                            if p + 4 > msg.len() {
                                return Err("the server sent a bad data row.".into());
                            }
                            let len = i32::from_be_bytes(msg[p..p + 4].try_into().unwrap());
                            p += 4;
                            if len < 0 {
                                row.push(String::new());
                            } else {
                                let end = p + len as usize;
                                let s = String::from_utf8_lossy(&msg[p..end]).to_string();
                                p = end;
                                row.push(s);
                            }
                        }
                        rows.push(row);
                    }
                    b'E' => return Err(pg_error(&msg)),
                    b'Z' => return Ok((names, rows)),
                    _ => {}
                }
            }
        }
    }

    fn pg_error(msg: &[u8]) -> String {
        // ErrorResponse: 'S' severity, 'C' code, 'M' message, terminated by 0
        let mut code = String::new();
        let mut text = String::new();
        let mut p = 0usize;
        while p < msg.len() && msg[p] != 0 {
            let field = msg[p];
            p += 1;
            let start = p;
            while p < msg.len() && msg[p] != 0 {
                p += 1;
            }
            let val = String::from_utf8_lossy(&msg[start..p]).to_string();
            if field == b'C' {
                code = val.clone();
            }
            if field == b'M' {
                text = val;
            }
            p += 1;
        }
        if code.is_empty() {
            text
        } else {
            format!("{text} ({code})")
        }
    }
}

pub enum Conn {
    #[cfg(feature = "sqlite")]
    Sqlite(rusqlite::Connection),
    Postgres(pg::PgConn),
}

thread_local! {
    static DBS: RefCell<HashMap<String, Conn>> = RefCell::new(HashMap::new());
}

fn dbs_with<R>(f: impl FnOnce(&mut HashMap<String, Conn>) -> R) -> R {
    DBS.with(|d| f(&mut d.borrow_mut()))
}

fn is_postgres(name: &str) -> bool {
    dbs_with(|m| matches!(m.get(name), Some(Conn::Postgres(_))))
}

/// Convert a PostgreSQL text value to a Lykaon value. The simple query
/// protocol returns everything as text; NULL arrives as an empty string.
fn pg_text_value(s: String) -> Value {
    if s.is_empty() {
        return Value::Missing;
    }
    if s.eq_ignore_ascii_case("t") || s.eq_ignore_ascii_case("true") {
        return Value::Boolean(true);
    }
    if s.eq_ignore_ascii_case("f") || s.eq_ignore_ascii_case("false") {
        return Value::Boolean(false);
    }
    if let Ok(n) = s.parse::<i64>() {
        return Value::Whole(n);
    }
    if let Ok(x) = s.parse::<f64>() {
        return Value::Dec(x);
    }
    Value::Text(s)
}

/// A SQL literal for the simple query protocol. Text is quoted and single
/// quotes are doubled; the book's "values are parameters" rule is kept for
/// SQLite (which binds), while PostgreSQL's simple protocol has no bind
/// parameter, so this is the safe literal form.
fn pg_literal(v: &Value) -> Result<String, String> {
    match v {
        Value::Whole(n) => Ok(n.to_string()),
        Value::Dec(x) => Ok(x.to_string()),
        Value::Text(s) => Ok(format!("'{}'", s.replace('\'', "''"))),
        Value::Boolean(b) => Ok(if *b { "TRUE" } else { "FALSE" }.to_string()),
        Value::Missing => Ok("NULL".to_string()),
        _ => Err("a value of this kind cannot be stored in a database.".into()),
    }
}

fn read_table_postgres(name: &str, table: &str) -> Result<Table, String> {
    let sql = format!("SELECT * FROM {}", quote_ident(table));
    let (names, rows) = dbs_with(|m| match m.get_mut(name) {
        Some(Conn::Postgres(c)) => c.query(&sql),
        #[cfg(feature = "sqlite")]
        Some(Conn::Sqlite(_)) => Err("that database is not PostgreSQL.".into()),
        None => Err("the database is not open.".into()),
    })?;
    let rows: Vec<Vec<Value>> = rows
        .into_iter()
        .map(|r| r.into_iter().map(pg_text_value).collect())
        .collect();
    let mut columns = Vec::with_capacity(names.len());
    for ci in 0..names.len() {
        let mut kind: Option<Kind> = None;
        let mut any_missing = false;
        for r in &rows {
            match &r[ci] {
                Value::Missing => any_missing = true,
                Value::Whole(_) => {
                    kind = Some(match kind {
                        Some(Kind::Decimal) => Kind::Decimal,
                        _ => Kind::Whole,
                    })
                }
                Value::Dec(_) => kind = Some(Kind::Decimal),
                Value::Boolean(_) => {
                    kind = Some(match kind {
                        Some(Kind::Text) => Kind::Text,
                        _ => Kind::Boolean,
                    })
                }
                Value::Text(_) => kind = Some(Kind::Text),
                _ => kind = Some(Kind::Text),
            }
        }
        let inner = kind.unwrap_or(Kind::Text);
        let kind = if any_missing {
            Kind::Maybe(Box::new(inner))
        } else {
            inner
        };
        columns.push(crate::data::Column {
            name: names[ci].clone(),
            kind,
        });
    }
    Ok(Table { columns, rows })
}

fn insert_postgres(db: &str, table: &str, parts: &[(String, Value)]) -> Result<(), String> {
    let cols: Vec<String> = parts.iter().map(|(n, _)| quote_ident(n)).collect();
    let vals: Vec<String> = parts
        .iter()
        .map(|(_, v)| pg_literal(v))
        .collect::<Result<_, _>>()?;
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(table),
        cols.join(", "),
        vals.join(", ")
    );
    dbs_with(|m| match m.get_mut(db) {
        Some(Conn::Postgres(c)) => c.query(&sql).map(|_| ()),
        #[cfg(feature = "sqlite")]
        Some(Conn::Sqlite(_)) => Err("that database is not PostgreSQL.".into()),
        None => Err("the database is not open.".into()),
    })
}

fn execute_sql_postgres(db: &str, sql: &str) -> Result<(), String> {
    dbs_with(|m| match m.get_mut(db) {
        Some(Conn::Postgres(c)) => c.query(sql).map(|_| ()),
        #[cfg(feature = "sqlite")]
        Some(Conn::Sqlite(_)) => Err("that database is not PostgreSQL.".into()),
        None => Err("the database is not open.".into()),
    })
}

pub fn open_sqlite(name: &str, path: &str) -> Result<(), String> {
    #[cfg(feature = "sqlite")]
    {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| format!("the database file {path} cannot be opened: {e}"))?;
        dbs_with(|m| {
            m.insert(name.to_string(), Conn::Sqlite(conn));
        });
        Ok(())
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (name, path);
        Err("this build has no SQLite driver.".into())
    }
}

pub fn open_postgres(
    name: &str,
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    database: &str,
) -> Result<(), String> {
    let conn = pg::PgConn::connect(host, port, user, password, database)?;
    dbs_with(|m| {
        m.insert(name.to_string(), Conn::Postgres(conn));
    });
    Ok(())
}

pub fn close(name: &str) {
    dbs_with(|m| {
        m.remove(name);
    });
}

pub fn names() -> Vec<String> {
    dbs_with(|m| m.keys().cloned().collect())
}

pub fn is_open(name: &str) -> bool {
    dbs_with(|m| m.contains_key(name))
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

#[cfg(feature = "sqlite")]
fn sql_value(v: &Value) -> Result<rusqlite::types::Value, String> {
    Ok(match v {
        Value::Whole(n) => rusqlite::types::Value::Integer(*n),
        Value::Dec(x) => rusqlite::types::Value::Real(*x),
        Value::Text(s) => rusqlite::types::Value::Text(s.clone()),
        Value::Boolean(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        Value::Missing => rusqlite::types::Value::Null,
        _ => return Err("a database value must be a number, text, yes/no or missing.".into()),
    })
}

#[cfg(feature = "sqlite")]
fn value_from_sql(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Value::Missing,
        ValueRef::Integer(i) => Value::Whole(i),
        ValueRef::Real(x) => Value::Dec(x),
        ValueRef::Text(s) => Value::Text(String::from_utf8_lossy(s).to_string()),
        ValueRef::Blob(b) => Value::Text(String::from_utf8_lossy(b).to_string()),
    }
}

/// M5-S33: read a whole table. Kinds come from the declared column types
/// when SQLite gives them, otherwise from the values.
pub fn read_table(name: &str, table: &str) -> Result<Table, String> {
    if is_postgres(name) {
        return read_table_postgres(name, table);
    }
    #[cfg(feature = "sqlite")]
    {
        return dbs_with(|m| {
            let conn = match m.get_mut(name) {
                Some(Conn::Sqlite(c)) => c,
                Some(_) => return Err("that database is not SQLite.".to_string()),
                None => return Err("the database is not open.".to_string()),
            };
            let sql = format!("SELECT * FROM {}", quote_ident(table));
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("the table {table} cannot be read: {e}"))?;
            let count = stmt.column_count();
            let names: Vec<String> = (0..count)
                .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                .collect();
            let mut rows: Vec<Vec<Value>> = Vec::new();
            let mut iter = stmt
                .query([])
                .map_err(|e| format!("the table {table} cannot be read: {e}"))?;
            while let Some(row) = iter
                .next()
                .map_err(|e| format!("the table {table} cannot be read: {e}"))?
            {
                let mut out = Vec::with_capacity(count);
                for i in 0..count {
                    let v = row.get_ref(i).map_err(|_| "a column cannot be read.".to_string())?;
                    out.push(value_from_sql(v));
                }
                rows.push(out);
            }
            // Infer kinds from the non-missing cells, as Load does.
            let mut columns = Vec::with_capacity(count);
            for ci in 0..count {
                let mut kind: Option<Kind> = None;
                let mut any_missing = false;
                for r in &rows {
                    match &r[ci] {
                        Value::Missing => any_missing = true,
                        Value::Whole(_) => {
                            kind = Some(match kind {
                                Some(Kind::Decimal) => Kind::Decimal,
                                _ => Kind::Whole,
                            })
                        }
                        Value::Dec(_) => kind = Some(Kind::Decimal),
                        Value::Boolean(_) => {
                            kind = Some(match kind {
                                Some(Kind::Text) => Kind::Text,
                                _ => Kind::Boolean,
                            })
                        }
                        Value::Text(_) => kind = Some(Kind::Text),
                        _ => kind = Some(Kind::Text),
                    }
                }
                let inner = kind.unwrap_or(Kind::Text);
                let kind = if any_missing {
                    Kind::Maybe(Box::new(inner))
                } else {
                    inner
                };
                columns.push(crate::data::Column {
                    name: names[ci].clone(),
                    kind,
                });
            }
            Ok(Table { columns, rows })
        });
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (name, table);
        Err("this build has no SQLite driver.".into())
    }
}

pub fn insert(db: &str, table: &str, parts: &[(String, Value)]) -> Result<(), String> {
    if is_postgres(db) {
        return insert_postgres(db, table, parts);
    }
    #[cfg(feature = "sqlite")]
    {
        return dbs_with(|m| {
            let conn = match m.get_mut(db) {
                Some(Conn::Sqlite(c)) => c,
                Some(_) => return Err("that database is not SQLite.".to_string()),
                None => return Err("the database is not open.".to_string()),
            };
            let cols: Vec<String> = parts.iter().map(|(n, _)| quote_ident(n)).collect();
            let marks: Vec<String> = (1..=parts.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                quote_ident(table),
                cols.join(", "),
                marks.join(", ")
            );
            let vals: Vec<rusqlite::types::Value> = parts
                .iter()
                .map(|(_, v)| sql_value(v))
                .collect::<Result<_, _>>()?;
            conn.execute(&sql, rusqlite::params_from_iter(vals))
                .map_err(|e| format!("the row cannot be added: {e}"))?;
            Ok(())
        });
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (db, table, parts);
        Err("this build has no SQLite driver.".into())
    }
}

pub fn delete_rows(db: &str, table: &str, rowids: &[String]) -> Result<(), String> {
    if rowids.is_empty() {
        return Ok(());
    }
    dbs_with(|m| {
        let conn = match m.get_mut(db) {
            Some(c) => c,
            None => return Err("the database is not open.".to_string()),
        };
        for id in rowids {
            match conn {
                #[cfg(feature = "sqlite")]
                Conn::Sqlite(c) => {
                    let id: i64 = id
                        .parse()
                        .map_err(|_| "a row id cannot be read.".to_string())?;
                    let sql = format!("DELETE FROM {} WHERE rowid = ?1", quote_ident(table));
                    c.execute(&sql, [id])
                        .map_err(|e| format!("the rows cannot be removed: {e}"))?;
                }
                Conn::Postgres(c) => {
                    let sql = format!(
                        "DELETE FROM {} WHERE ctid = '{}'",
                        quote_ident(table),
                        id.replace('\'', "''")
                    );
                    c.query(&sql)
                        .map_err(|e| format!("the rows cannot be removed: {e}"))?;
                }
            }
        }
        Ok(())
    })
}

/// Read the rowids and cells needed to evaluate a where condition.
pub fn select_rows(
    db: &str,
    table: &str,
) -> Result<(Vec<String>, Vec<Vec<Value>>, Vec<String>), String> {
    if is_postgres(db) {
        return select_rows_postgres(db, table);
    }
    #[cfg(feature = "sqlite")]
    {
        return dbs_with(|m| {
            let conn = match m.get_mut(db) {
                Some(Conn::Sqlite(c)) => c,
                Some(_) => return Err("that database is not SQLite.".to_string()),
                None => return Err("the database is not open.".to_string()),
            };
            let sql = format!("SELECT rowid, * FROM {}", quote_ident(table));
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("the table {table} cannot be read: {e}"))?;
            let count = stmt.column_count();
            let names: Vec<String> = (1..count)
                .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                .collect();
            let mut rows = Vec::new();
            let mut ids = Vec::new();
            let mut iter = stmt
                .query([])
                .map_err(|e| format!("the table {table} cannot be read: {e}"))?;
            while let Some(row) = iter
                .next()
                .map_err(|e| format!("the table {table} cannot be read: {e}"))?
            {
                let id = row.get::<_, i64>(0).unwrap_or(0);
                let mut out = Vec::with_capacity(count - 1);
                for i in 1..count {
                    let v = row.get_ref(i).map_err(|_| "a column cannot be read.".to_string())?;
                    out.push(value_from_sql(v));
                }
                ids.push(id.to_string());
                rows.push(out);
            }
            Ok((names, rows, ids))
        });
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (db, table);
        Err("this build has no SQLite driver.".into())
    }
}

fn select_rows_postgres(
    db: &str,
    table: &str,
) -> Result<(Vec<String>, Vec<Vec<Value>>, Vec<String>), String> {
    let sql = format!("SELECT ctid, * FROM {}", quote_ident(table));
    let (names, rows) = dbs_with(|m| match m.get_mut(db) {
        Some(Conn::Postgres(c)) => c.query(&sql),
        #[cfg(feature = "sqlite")]
        Some(Conn::Sqlite(_)) => Err("that database is not PostgreSQL.".into()),
        None => Err("the database is not open.".into()),
    })?;
    if names.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let names = names[1..].to_vec();
    let mut ids = Vec::with_capacity(rows.len());
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        if r.is_empty() {
            continue;
        }
        ids.push(r[0].clone());
        out.push(r[1..].iter().map(|s| pg_text_value(s.clone())).collect());
    }
    Ok((names, out, ids))
}

pub fn update_cell(
    db: &str,
    table: &str,
    column: &str,
    value: &Value,
    rowid: &str,
) -> Result<(), String> {
    if is_postgres(db) {
        return update_cell_postgres(db, table, column, value, rowid);
    }
    #[cfg(feature = "sqlite")]
    {
        return dbs_with(|m| {
            let conn = match m.get_mut(db) {
                Some(Conn::Sqlite(c)) => c,
                Some(_) => return Err("that database is not SQLite.".to_string()),
                None => return Err("the database is not open.".to_string()),
            };
            let v = sql_value(value)?;
            let id: i64 = rowid
                .parse()
                .map_err(|_| "a row id cannot be read.".to_string())?;
            let sql = format!(
                "UPDATE {} SET {} = ?1 WHERE rowid = ?2",
                quote_ident(table),
                quote_ident(column)
            );
            conn.execute(&sql, rusqlite::params![v, id])
                .map_err(|e| format!("the row cannot be set: {e}"))?;
            Ok(())
        });
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (db, table, column, value, rowid);
        Err("this build has no SQLite driver.".into())
    }
}

fn update_cell_postgres(
    db: &str,
    table: &str,
    column: &str,
    value: &Value,
    rowid: &str,
) -> Result<(), String> {
    let v = pg_literal(value)?;
    let sql = format!(
        "UPDATE {} SET {} = {} WHERE ctid = '{}'",
        quote_ident(table),
        quote_ident(column),
        v,
        rowid.replace('\'', "''")
    );
    dbs_with(|m| match m.get_mut(db) {
        Some(Conn::Postgres(c)) => c.query(&sql).map(|_| ()),
        #[cfg(feature = "sqlite")]
        Some(Conn::Sqlite(_)) => Err("that database is not PostgreSQL.".into()),
        None => Err("the database is not open.".into()),
    })
}

/// Begin a transaction on every open database (M5-S37).
pub fn begin_all() -> Result<(), String> {
    dbs_with(|m| {
        for (_, c) in m.iter_mut() {
            match c {
                #[cfg(feature = "sqlite")]
                Conn::Sqlite(conn) => {
                    conn.execute_batch("BEGIN")
                        .map_err(|e| format!("the transaction cannot begin: {e}"))?;
                }
                Conn::Postgres(conn) => {
                    conn.query("BEGIN")
                        .map_err(|e| format!("the transaction cannot begin: {e}"))?;
                }
            }
        }
        Ok(())
    })
}

pub fn commit_all() -> Result<(), String> {
    dbs_with(|m| {
        for (_, c) in m.iter_mut() {
            match c {
                #[cfg(feature = "sqlite")]
                Conn::Sqlite(conn) => {
                    conn.execute_batch("COMMIT")
                        .map_err(|e| format!("the transaction cannot commit: {e}"))?;
                }
                Conn::Postgres(conn) => {
                    conn.query("COMMIT")
                        .map_err(|e| format!("the transaction cannot commit: {e}"))?;
                }
            }
        }
        Ok(())
    })
}

pub fn rollback_all() {
    dbs_with(|m| {
        for (_, c) in m.iter_mut() {
            match c {
                #[cfg(feature = "sqlite")]
                Conn::Sqlite(conn) => {
                    let _ = conn.execute_batch("ROLLBACK");
                }
                Conn::Postgres(conn) => {
                    let _ = conn.query("ROLLBACK");
                }
            }
        }
    });
}

/// Run one SQL script (no result rows) against an open database. Used by
/// the migrate command (M5-S38).
pub fn execute_sql(db: &str, sql: &str) -> Result<(), String> {
    if is_postgres(db) {
        return execute_sql_postgres(db, sql);
    }
    #[cfg(feature = "sqlite")]
    {
        return dbs_with(|m| {
            let conn = match m.get_mut(db) {
                Some(Conn::Sqlite(c)) => c,
                Some(_) => return Err("that database is not SQLite.".to_string()),
                None => return Err("the database is not open.".to_string()),
            };
            conn.execute_batch(sql)
                .map_err(|e| format!("the SQL cannot run: {e}"))
        });
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (db, sql);
        Err("this build has no SQLite driver.".into())
    }
}

fn sql_text(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Whole(n) => n.to_string(),
        Value::Dec(x) => x.to_string(),
        Value::Boolean(b) => if *b { "1" } else { "0" }.to_string(),
        Value::Missing => String::new(),
        other => crate::data::cell_text(other),
    }
}

/// Run one query and return its rows as text (M5-S38 status).
pub fn query_strings(db: &str, sql: &str) -> Result<Vec<Vec<String>>, String> {
    #[cfg(feature = "sqlite")]
    {
        return dbs_with(|m| {
            let conn = match m.get_mut(db) {
                Some(Conn::Sqlite(c)) => c,
                Some(_) => return Err("that database is not SQLite.".to_string()),
                None => return Err("the database is not open.".to_string()),
            };
            let mut stmt = conn
                .prepare(sql)
                .map_err(|e| format!("the query cannot run: {e}"))?;
            let count = stmt.column_count();
            let mut rows = Vec::new();
            let mut iter = stmt.query([]).map_err(|e| e.to_string())?;
            while let Some(row) = iter.next().map_err(|e| e.to_string())? {
                let mut out = Vec::with_capacity(count);
                for i in 0..count {
                    let v = row
                        .get_ref(i)
                        .map_err(|_| "a column cannot be read.".to_string())?;
                    out.push(sql_text(&value_from_sql(v)));
                }
                rows.push(out);
            }
            Ok(rows)
        });
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (db, sql);
        Err("this build has no SQLite driver.".into())
    }
}
