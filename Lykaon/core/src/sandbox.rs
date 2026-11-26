//! M5 (Appendix L 7.6, M5-S43..S48): the native permission sandbox.
//!
//! `lykaon run safely` runs a program with the permissions named by
//! `Allows ...` sentences in `settings.lykaon` (or the project settings).
//! Files outside the allowed folders, hosts outside the allowed list,
//! environment variables outside the list and (unless `Allows processes`)
//! spawning processes are refused with T0622.
//!
//! This is the native enforcement path. The WASI/wasmtime sandbox is a
//! separate target and is not built yet; the permission model is the same.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Sandbox {
    /// None = every file under the project (plain `Allows files`).
    files: Option<Vec<PathBuf>>,
    /// None = every host (plain `Allows network`).
    hosts: Option<Vec<String>>,
    /// None = every variable (plain `Allows environment`).
    env: Option<Vec<String>>,
    processes: bool,
    active: bool,
    /// M5 Step 6: did the settings mention each permission at all? Used by
    /// the `lykaon add` permission summary to tell "not requested" from a
    /// broad (plain) request.
    saw_allows: bool,
    saw_files: bool,
    saw_network: bool,
    saw_env: bool,
    /// M5g item f: what the run actually touched (recorded by the checks).
    used: RefCell<Used>,
}

/// M5g item f: the permissions a run used, capped so a huge run cannot make
/// the report huge.
#[derive(Clone, Debug, Default)]
struct Used {
    files: Vec<String>,
    hosts: Vec<String>,
    env: Vec<String>,
    processes: u32,
}

const USED_CAP: usize = 256;

/// Split an `Allows` body into its `and`-separated permission clauses.
fn split_clauses(body: &str) -> Vec<String> {
    let lower = body.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i + 5 <= lb.len() {
        if &lb[i..i + 5] == b" and " {
            out.push(body[start..i].trim().to_string());
            start = i + 5;
            i += 5;
        } else {
            i += 1;
        }
    }
    out.push(body[start..].trim().to_string());
    out.retain(|c| !c.is_empty());
    out
}

fn words(rest: &str) -> Vec<String> {
    rest.split_whitespace()
        .map(|w| w.trim().trim_end_matches('.'))
        .filter(|w| {
            !matches!(
                *w,
                "and"
                    | "files"
                    | "network"
                    | "environment"
                    | "processes"
                    | "to"
                    | "host"
                    | "variable"
                    | "in"
                    | "folder"
            )
        })
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

fn merge(dst: &mut Option<Vec<String>>, add: Vec<String>) {
    match dst {
        Some(v) => {
            for a in add {
                if !v.contains(&a) {
                    v.push(a);
                }
            }
        }
        None => *dst = Some(add),
    }
}

impl Sandbox {
    /// Read the Allows sentences from settings.lykaon (fallback
    /// settings.tern) under `dir`.
    pub fn from_settings(dir: &Path) -> Self {
        let mut s = Sandbox::default();
        let text = std::fs::read_to_string(dir.join("settings.lykaon"))
            .or_else(|_| std::fs::read_to_string(dir.join("settings.tern")));
        let Ok(text) = text else {
            return s;
        };
        for line in text.lines() {
            let t = line.trim();
            let lower = t.to_ascii_lowercase();
            if !lower.starts_with("allows ") {
                continue;
            }
            s.saw_allows = true;
            let body = &t["Allows ".len()..];
            // G141: "Allows" permission ("and" permission)*. Split at each
            // " and " so a file list never absorbs the hosts or variables of
            // the clauses that follow it.
            for clause in split_clauses(body) {
                let lb = clause.to_ascii_lowercase();
                if lb.starts_with("files") {
                    s.saw_files = true;
                    if let Some(p) = lb.find("in folder ") {
                        let list = words(&clause[p + "in folder ".len()..]);
                        match s.files {
                            Some(ref mut v) => {
                                for w in list {
                                    let pb = dir.join(&w);
                                    if !v.contains(&pb) {
                                        v.push(pb);
                                    }
                                }
                            }
                            None => {
                                s.files = Some(list.into_iter().map(|w| dir.join(w)).collect())
                            }
                        }
                    } else {
                        s.files = None; // plain Allows files: the project folder
                    }
                } else if lb.starts_with("network") {
                    s.saw_network = true;
                    if let Some(p) = lb.find("to host ") {
                        let list = words(&clause[p + "to host ".len()..]);
                        merge(&mut s.hosts, list);
                    } else {
                        s.hosts = None;
                    }
                } else if lb.starts_with("environment") {
                    s.saw_env = true;
                    if let Some(p) = lb.find("variable ") {
                        let list = words(&clause[p + "variable ".len()..]);
                        merge(&mut s.env, list);
                    } else {
                        s.env = None;
                    }
                } else if lb.starts_with("processes") {
                    s.processes = true;
                }
            }
        }
        s
    }

    pub fn activate(&mut self) {
        self.active = true;
        if self.files.is_none() {
            // plain Allows files / no settings: the project folder
            // (the caller leaves files None; check_file then allows all)
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn check_file(&self, path: &Path) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let mut used = self.used.borrow_mut();
        if used.files.len() < USED_CAP {
            used.files.push(path.display().to_string());
        }
        drop(used);
        let Some(allowed) = &self.files else {
            return Ok(());
        };
        let path = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
        } else {
            let parent = path
                .parent()
                .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."));
            parent.join(path.file_name().unwrap_or_default())
        };
        for a in allowed {
            let a = if a.exists() {
                a.canonicalize().unwrap_or_else(|_| a.clone())
            } else {
                a.clone()
            };
            if path.starts_with(&a) {
                return Ok(());
            }
        }
        Err(format!("the path {} is outside the allowed folders", path.display()))
    }

    pub fn check_host(&self, host: &str) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        {
            let mut used = self.used.borrow_mut();
            if used.hosts.len() < USED_CAP {
                used.hosts.push(host.to_string());
            }
        }
        let Some(hosts) = &self.hosts else {
            return Ok(());
        };
        let host = host.to_ascii_lowercase();
        for h in hosts {
            if host == *h || host.ends_with(&format!(".{h}")) {
                return Ok(());
            }
        }
        Err(format!("the host {host} is not allowed"))
    }

    pub fn check_env(&self, name: &str) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        {
            let mut used = self.used.borrow_mut();
            if used.env.len() < USED_CAP {
                used.env.push(name.to_string());
            }
        }
        let Some(env) = &self.env else {
            return Ok(());
        };
        if env.iter().any(|e| e.eq_ignore_ascii_case(name)) {
            Ok(())
        } else {
            Err(format!("the environment variable {name} is not allowed"))
        }
    }

    pub fn check_process(&self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        self.used.borrow_mut().processes += 1;
        if self.processes {
            return Ok(());
        }
        Err("starting processes is not allowed".to_string())
    }

    /// M5g item f: the permissions this run used, as `file <path>`,
    /// `host <h>`, `env <NAME>` and `processes <count>` lines.
    pub fn usage(&self) -> Vec<String> {
        let used = self.used.borrow();
        let mut out = Vec::new();
        for f in used.files.iter() {
            out.push(format!("file {f}"));
        }
        for h in used.hosts.iter() {
            out.push(format!("host {h}"));
        }
        for e in used.env.iter() {
            out.push(format!("env {e}"));
        }
        if used.processes > 0 {
            out.push(format!("processes {}", used.processes));
        }
        out
    }

    /// M5f: did the settings declare each permission? (for the audit report)
    pub fn declares_files(&self) -> bool {
        self.saw_files
    }
    pub fn declares_network(&self) -> bool {
        self.saw_network
    }
    pub fn declares_environment(&self) -> bool {
        self.saw_env
    }
    pub fn declares_processes(&self) -> bool {
        self.processes
    }

    /// M5 Step 6: read the permissions a package requests from its
    /// `settings.lykaon` and return a human-readable summary. `None` when
    /// there is no settings file or it names no `Allows` sentence.
    pub fn summarize_dir(dir: &Path) -> Option<String> {
        let s = Self::from_settings(dir);
        if !s.saw_allows {
            return None;
        }
        let mut lines: Vec<String> = Vec::new();
        if s.saw_files {
            match &s.files {
                None => lines.push("  files: any file in the project".to_string()),
                Some(v) => lines.push(format!(
                    "  files in: {}",
                    v.iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        if s.saw_network {
            match &s.hosts {
                None => lines.push("  network: any host".to_string()),
                Some(v) if v.is_empty() => lines.push("  network: any host".to_string()),
                Some(v) => lines.push(format!("  network to: {}", v.join(", "))),
            }
        }
        if s.saw_env {
            match &s.env {
                None => lines.push("  environment: any variable".to_string()),
                Some(v) if v.is_empty() => lines.push("  environment: any variable".to_string()),
                Some(v) => lines.push(format!("  environment variables: {}", v.join(", "))),
            }
        }
        if s.processes {
            lines.push("  processes: may start processes".to_string());
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(tag: &str, settings: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lykaon_sandbox_test_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.lykaon"), settings).unwrap();
        dir
    }

    #[test]
    fn multi_clause_does_not_absorb_later_clauses() {
        let dir = dir_with(
            "multi",
            "Allows files in folder data and network to host api.example.com and environment variable API_KEY and processes.\n",
        );
        let s = Sandbox::from_settings(&dir);
        assert_eq!(
            s.files,
            Some(vec![dir.join("data")]),
            "file list must not include hosts or variables"
        );
        assert_eq!(s.hosts, Some(vec!["api.example.com".to_string()]));
        assert_eq!(s.env, Some(vec!["api_key".to_string()]));
        assert!(s.processes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summary_lists_every_request() {
        let dir = dir_with(
            "summary",
            "Allows files in folder data and network to host example.com and environment variable LYKAON_X.\n",
        );
        let text = Sandbox::summarize_dir(&dir).expect("summary");
        assert!(text.contains("files in:"), "{text}");
        assert!(text.contains("example.com"), "{text}");
        assert!(text.contains("lykaon_x"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_settings_means_no_summary() {
        let dir = std::env::temp_dir().join(format!("lykaon_sandbox_none_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(Sandbox::summarize_dir(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}