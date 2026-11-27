//! M6b local package registry.
//!
//! A `.ship` is a gzip-compressed JSON package: name, version and files
//! (path + base64). `index.json` maps names to versions and SHA-256. No
//! install scripts and no network code execution.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn publish(folder: &Path, registry: &Path) -> Result<(String, String), String> {
    let toml = std::fs::read_to_string(folder.join("Lykaon.toml"))
        .unwrap_or_default();
    let name = field(&toml, "Name is")
        .or_else(|| folder.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "package".to_string());
    let version = field(&toml, "Version is").unwrap_or_else(|| "0.1.0".to_string());
    let mut files = Vec::new();
    collect(folder, folder, &mut files)?;
    let body = json!({
        "name": name,
        "version": version,
        "files": files.iter().map(|(p,d)| json!({"path": p, "data": b64(d)})).collect::<Vec<_>>(),
    });
    let raw = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).map_err(|e| e.to_string())?;
    let ship = enc.finish().map_err(|e| e.to_string())?;
    let hash = sha256(&ship);
    let dir = registry.join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{version}.ship"));
    std::fs::write(&path, &ship).map_err(|e| e.to_string())?;
    let mut index = read_index(registry);
    let packages = index
        .as_object_mut()
        .ok_or_else(|| "registry index is not an object".to_string())?
        .entry("packages")
        .or_insert_with(|| json!({}));
    let entry = packages
        .as_object_mut()
        .ok_or_else(|| "registry packages is not an object".to_string())?
        .entry(name.clone())
        .or_insert_with(|| json!({"versions": {}, "latest": ""}));
    entry["versions"][&version] = json!(hash);
    entry["latest"] = json!(version);
    write_index(registry, &index)?;
    Ok((name, version))
}

pub fn add(name: &str, registry: &Path, dest: &Path) -> Result<PathBuf, String> {
    let index = read_index(registry);
    let entry = index
        .get("packages")
        .and_then(|p| p.get(name))
        .ok_or_else(|| format!("package {name} is not in the registry"))?;
    let version = entry["latest"].as_str().unwrap_or("").to_string();
    let hash = entry["versions"][&version].as_str().unwrap_or("").to_string();
    let ship = registry.join(name).join(format!("{version}.ship"));
    let raw = std::fs::read(&ship).map_err(|e| format!("cannot read {}: {e}", ship.display()))?;
    if sha256(&raw) != hash {
        return Err(format!("checksum mismatch for {name} {version}"));
    }
    let mut gz = flate2::read::GzDecoder::new(&raw[..]);
    let mut text = String::new();
    gz.read_to_string(&mut text).map_err(|e| e.to_string())?;
    let body: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let out = dest.join(name).join(&version);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    if let Some(files) = body["files"].as_array() {
        for f in files {
            let p = f["path"].as_str().unwrap_or("");
            if !safe_rel_path(p) {
                return Err(format!("package {name} contains an unsafe path: {p}"));
            }
            let data = unb64(f["data"].as_str().unwrap_or(""))?;
            let target = out.join(p);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, data).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

pub fn status(registry: &Path) -> String {
    let index = read_index(registry);
    serde_json::to_string_pretty(&index).unwrap_or_else(|_| "{}".to_string())
}

fn field(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix(key) {
            let v = rest.trim().trim_end_matches('.').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    for e in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name == ".git" || name == "target" || name == "registry" {
                continue;
            }
            collect(base, &p, out)?;
        } else if p.is_file() {
            let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            out.push((rel, std::fs::read(&p).map_err(|e| e.to_string())?));
        }
    }
    Ok(())
}

/// A package path must stay inside the destination folder: no absolute
/// paths, no drive/root prefixes, no `..` components.
fn safe_rel_path(p: &str) -> bool {
    use std::path::Component;
    let path = Path::new(p);
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn read_index(registry: &Path) -> Value {
    let p = registry.join("index.json");
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({"packages": {}}))
}

fn write_index(registry: &Path, index: &Value) -> Result<(), String> {
    std::fs::create_dir_all(registry).map_err(|e| e.to_string())?;
    let p = registry.join("index.json");
    std::fs::write(p, serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn sha256(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

fn b64(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        out.push(B64[(b[0] >> 2) as usize] as char);
        out.push(B64[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 { B64[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[(b[2] & 0x3f) as usize] as char } else { '=' });
    }
    out
}

fn unb64(s: &str) -> Result<Vec<u8>, String> {
    let mut rev = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::new();
    let bytes: Vec<u8> = s.bytes().filter(|b| *b != b'=').collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return Err("bad base64".to_string());
        }
        let mut n = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            let v = rev[*b as usize];
            if v == 255 {
                return Err("bad base64".to_string());
            }
            n |= (v as u32) << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 { out.push((n >> 8) as u8); }
        if chunk.len() > 3 { out.push(n as u8); }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_refuses_a_path_that_escapes_the_destination() {
        let root = std::env::temp_dir().join(format!("lykaon_reg_escape_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let reg = root.join("registry");
        std::fs::create_dir_all(reg.join("evil")).unwrap();
        let body = json!({
            "name": "evil",
            "version": "1.0.0",
            "files": [{"path": "../escape.txt", "data": b64(b"x")}],
        });
        let raw = serde_json::to_vec(&body).unwrap();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw).unwrap();
        let ship = enc.finish().unwrap();
        std::fs::write(reg.join("evil").join("1.0.0.ship"), &ship).unwrap();
        let index = json!({
            "packages": {
                "evil": {"versions": {"1.0.0": sha256(&ship)}, "latest": "1.0.0"}
            }
        });
        std::fs::write(reg.join("index.json"), serde_json::to_vec(&index).unwrap()).unwrap();
        let dest = root.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let err = add("evil", &reg, &dest).unwrap_err();
        assert!(err.contains("unsafe path"), "{err}");
        assert!(!root.join("escape.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}