//! M5g item c: trace v1 event stream.
//!
//! A trace records the outside-world reads a program makes, in execution
//! order, so a later run can feed the same values back. The language is not
//! involved: these are interpreter-level hooks, active only when the caller
//! asks for a recording or a replay.
//!
//! The JSON encoding lives in the front end (`src/trace.rs`); this module is
//! plain data so `lykaon-core` stays serde-free.

/// M5g item b: the deterministic-mode configuration an embedding runtime
/// passes in. The CLI sets these for a native child; the interpreter paths
/// pass the same values explicitly.
pub fn env_config() -> (bool, u64) {
    let deterministic = std::env::var("LYKAON_DETERMINISTIC")
        .map(|v| v == "1")
        .unwrap_or(false);
    let seed = std::env::var("LYKAON_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(42);
    (deterministic, seed)
}

/// One recorded outside-world read.
#[derive(Clone, Debug, PartialEq)]
pub enum TraceEvent {
    /// The whole standard input the run was given (written first).
    Stdin(String),
    /// A `now` read: nanoseconds since the Unix epoch and the zone name.
    Clock { nanos: i128, zone: String },
    /// A `today` read: (year, month 1..12, day 1..31).
    Today { year: i64, month: i64, day: i64 },
    /// One 64-bit draw from the program's random generator.
    Random(u64),
    /// A sleep request, in milliseconds.
    Sleep(u64),
    /// A text or bytes file read: path, byte length, SHA-256, and the
    /// content when it is small enough to store. A replay serves the stored
    /// content without touching the filesystem; without content it re-reads
    /// and verifies the length and hash.
    FileRead {
        path: String,
        len: u64,
        sha256: String,
        content: Option<Vec<u8>>,
    },
    /// A database open: name, driver, outcome and (on failure) the message.
    DbOpen {
        name: String,
        driver: String,
        ok: bool,
        message: String,
    },
    /// A whole-table database read (CSV snapshot, plus outcome).
    DbTable {
        db: String,
        table: String,
        csv: String,
        ok: bool,
        message: String,
    },
    /// A select for update/delete: CSV rows, rowids and outcome.
    DbRows {
        db: String,
        table: String,
        csv: String,
        ids: Vec<String>,
        ok: bool,
        message: String,
    },
    /// A database mutation or transaction step; a replay verifies and skips.
    DbWrite {
        op: String,
        db: String,
        table: String,
        ok: bool,
        message: String,
    },
    /// A session cookie value (server runs only).
    Cookie(String),
    /// A file write: path, byte length, SHA-256 of the bytes, and whether
    /// the write succeeded. A replay verifies the bytes and skips the write.
    FileWrite {
        path: String,
        len: u64,
        sha256: String,
        ok: bool,
    },
    /// A temporary file creation: the generated path (which contains the
    /// process id) and whether it succeeded. A replay reuses the recorded
    /// path and does not create or delete anything.
    TempFile { path: String, ok: bool },
    /// A directory listing: path and sorted entry names.
    DirList { path: String, names: Vec<String> },
    /// A filesystem mutation (make folder, remove, rename, zip, unzip).
    /// A replay returns the recorded outcome without touching the disk.
    FsOp {
        op: String,
        paths: Vec<String>,
        ok: bool,
    },
    /// An environment read: the value, or None when it was unset.
    Env { name: String, value: Option<String> },
    /// A successful HTTP response. `body` is stored when it is small enough
    /// (`TRACE_NET_BODY_MAX`); otherwise the replay re-issues the request and
    /// checks `len` and `sha256`.
    Net {
        method: String,
        url: String,
        status: u16,
        body: Option<Vec<u8>>,
        len: u64,
        sha256: String,
        headers: Vec<(String, String)>,
    },
    /// A failed HTTP request, recorded so replay fails the same way.
    NetFail {
        method: String,
        url: String,
        kind: String,
        message: String,
    },
}

/// The event families the replayer can be asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    Stdin,
    Clock,
    Today,
    Random,
    Sleep,
    DbOpen,
    DbTable,
    DbRows,
    DbWrite,
    Cookie,
    FileRead,
    FileWrite,
    TempFile,
    DirList,
    FsOp,
    Env,
    Net,
    NetFail,
}

impl TraceKind {
    pub fn name(self) -> &'static str {
        match self {
            TraceKind::Stdin => "stdin",
            TraceKind::Clock => "clock",
            TraceKind::Today => "today",
            TraceKind::Random => "random",
            TraceKind::Sleep => "sleep",
            TraceKind::DbOpen => "db_open",
            TraceKind::DbTable => "db_table",
            TraceKind::DbRows => "db_rows",
            TraceKind::DbWrite => "db_write",
            TraceKind::Cookie => "cookie",
            TraceKind::FileRead => "file_read",
            TraceKind::FileWrite => "file_write",
            TraceKind::TempFile => "temp_file",
            TraceKind::DirList => "dir_list",
            TraceKind::FsOp => "fs_op",
            TraceKind::Env => "environment",
            TraceKind::Net => "net",
            TraceKind::NetFail => "net_fail",
        }
    }
}

impl TraceEvent {
    pub fn kind(&self) -> TraceKind {
        match self {
            TraceEvent::Stdin(_) => TraceKind::Stdin,
            TraceEvent::Clock { .. } => TraceKind::Clock,
            TraceEvent::Today { .. } => TraceKind::Today,
            TraceEvent::Random(_) => TraceKind::Random,
            TraceEvent::Sleep(_) => TraceKind::Sleep,
            TraceEvent::DbOpen { .. } => TraceKind::DbOpen,
            TraceEvent::DbTable { .. } => TraceKind::DbTable,
            TraceEvent::DbRows { .. } => TraceKind::DbRows,
            TraceEvent::DbWrite { .. } => TraceKind::DbWrite,
            TraceEvent::Cookie(_) => TraceKind::Cookie,
            TraceEvent::FileRead { .. } => TraceKind::FileRead,
            TraceEvent::FileWrite { .. } => TraceKind::FileWrite,
            TraceEvent::TempFile { .. } => TraceKind::TempFile,
            TraceEvent::DirList { .. } => TraceKind::DirList,
            TraceEvent::FsOp { .. } => TraceKind::FsOp,
            TraceEvent::Env { .. } => TraceKind::Env,
            TraceEvent::Net { .. } => TraceKind::Net,
            TraceEvent::NetFail { .. } => TraceKind::NetFail,
        }
    }
}

/// What a run does with outside-world reads.
#[derive(Debug)]
pub enum TraceMode {
    /// Normal run: no recording, no replay.
    Off,
    /// Record every read, in order.
    Record(Vec<TraceEvent>),
    /// Feed reads back from the recorded stream.
    Replay { events: Vec<TraceEvent>, pos: usize },
}

impl TraceMode {
    pub fn is_off(&self) -> bool {
        matches!(self, TraceMode::Off)
    }

    pub fn is_recording(&self) -> bool {
        matches!(self, TraceMode::Record(_))
    }

    pub fn is_replay(&self) -> bool {
        matches!(self, TraceMode::Replay { .. })
    }

    /// Append an event when recording. A no-op otherwise.
    pub fn push(&mut self, event: TraceEvent) {
        if let TraceMode::Record(events) = self {
            events.push(event);
        }
    }

    /// Take the recorded events (empty when not recording).
    pub fn take_recorded(&mut self) -> Vec<TraceEvent> {
        match std::mem::replace(self, TraceMode::Off) {
            TraceMode::Record(events) => events,
            other => {
                *self = other;
                Vec::new()
            }
        }
    }

    /// Pop the next replay event, requiring the expected family. The error
    /// text is the divergence report; the caller wraps it in a fatal error.
    pub fn next(&mut self, expected: TraceKind) -> Result<TraceEvent, String> {
        let TraceMode::Replay { events, pos } = self else {
            return Err("not replaying".to_string());
        };
        if *pos >= events.len() {
            return Err(format!("the trace ended before a {} read", expected.name()));
        }
        let event = events[*pos].clone();
        if event.kind() != expected {
            return Err(format!(
                "expected a {} read but the trace has {}",
                expected.name(),
                event.kind().name()
            ));
        }
        *pos += 1;
        Ok(event)
    }

    /// Pop the next replay event without a family check (used for net, where
    /// success and failure share a call site).
    pub fn next_any(&mut self) -> Result<TraceEvent, String> {
        let TraceMode::Replay { events, pos } = self else {
            return Err("not replaying".to_string());
        };
        if *pos >= events.len() {
            return Err("the trace ended before the next read".to_string());
        }
        let event = events[*pos].clone();
        *pos += 1;
        Ok(event)
    }

    /// How many recorded events a replay never consumed.
    pub fn replay_left(&self) -> usize {
        match self {
            TraceMode::Replay { events, pos } => events.len() - *pos,
            _ => 0,
        }
    }

    /// The path of the next recorded file read, when that is what comes
    /// next. Used by a table load to pick the same candidate file.
    pub fn peek_file_path(&self) -> Option<String> {
        match self {
            TraceMode::Replay { events, pos } => match events.get(*pos) {
                Some(TraceEvent::FileRead { path, .. }) => Some(path.clone()),
                _ => None,
            },
            _ => None,
        }
    }
}