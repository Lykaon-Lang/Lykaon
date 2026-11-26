//! Shared types for the generated coverage manifests (tests/coverage/*.json).
//!
//! The manifests are written by Lykaon/tools/extract_coverage.py; this module
//! is the read side used by tests/coverage_manifest.rs. Parsing goes through
//! serde_json::Value so the test needs no extra dependencies.

use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub milestone: String,
    pub status: String,
    pub note: Option<String>,
    #[allow(dead_code)]
    pub name: String,
}

pub fn load(path: &Path) -> Vec<Item> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let v: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    v["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{} has no items array", path.display()))
        .iter()
        .map(|it| Item {
            id: it["id"].as_str().unwrap_or("").to_string(),
            milestone: it["milestone"].as_str().unwrap_or("").to_string(),
            status: it["status"].as_str().unwrap_or("").to_string(),
            note: it["note"].as_str().map(|s| s.to_string()),
            name: it["name"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}
