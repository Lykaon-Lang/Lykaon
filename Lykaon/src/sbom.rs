//! M5g item h: CycloneDX SBOM for a Lykaon build.
//!
//! Reads `Lykaon.toml` and `lykaon.lock` and emits a CycloneDX 1.5 JSON
//! document listing the project and its dependencies, with the lock's
//! checksums where they exist.
//!
//! The document is deterministic on purpose: components keep project/lock
//! order, the serial number is derived from the content (UUIDv5 shape) and
//! no timestamp is written. Feeding the same inputs twice gives the same
//! bytes, which is what a reproducible-build check needs.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// A deterministic UUID (version 5 layout) over the SBOM's identity.
fn content_uuid(seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    b[6] = (b[6] & 0x0f) | 0x50; // version 5 (name-based)
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

fn purl(name: &str, version: &str) -> String {
    format!("pkg:generic/{name}@{version}")
}

/// One resolved dependency line for the SBOM.
struct Dep {
    name: String,
    version: String,
    checksum: Option<String>,
}

pub fn sbom() -> String {
    sbom_in(std::path::Path::new("."))
}

/// Build the document for a project folder (the CLI uses `.`; tests use a
/// temporary folder so they never depend on the process's current directory).
pub fn sbom_in(dir: &std::path::Path) -> String {
    let mut name = "lykaon-project".to_string();
    let mut version = "0.0.0".to_string();
    let mut deps: Vec<Dep> = Vec::new();

    if let Ok(text) = std::fs::read_to_string(dir.join("Lykaon.toml")) {
        if let Ok(proj) = crate::packages::parse_project(&text) {
            if let Some(n) = &proj.name {
                name = n.clone();
            }
            if let Some((ma, mi, pa)) = proj.version {
                version = format!("{ma}.{mi}.{pa}");
            }
            for d in &proj.needs {
                deps.push(Dep {
                    name: d.name.clone(),
                    version: "unknown".to_string(),
                    checksum: None,
                });
            }
        }
    }

    if let Ok(text) = std::fs::read_to_string(dir.join("lykaon.lock")) {
        if let Ok(lock) = crate::packages::parse_lockfile(&text) {
            for dep in &lock.locked {
                let v = format!("{}.{}.{}", dep.version.0, dep.version.1, dep.version.2);
                match deps.iter_mut().find(|d| d.name == dep.name) {
                    Some(d) => {
                        d.version = v;
                        d.checksum = dep.checksum.clone();
                    }
                    None => deps.push(Dep {
                        name: dep.name.clone(),
                        version: v,
                        checksum: dep.checksum.clone(),
                    }),
                }
            }
        }
    }

    let root_ref = purl(&name, &version);
    let mut comps: Vec<Value> = Vec::new();
    let mut depends_on: Vec<String> = Vec::new();
    for d in &deps {
        let r = purl(&d.name, &d.version);
        depends_on.push(r.clone());
        let mut comp = json!({
            "type": "library",
            "bom-ref": &r,
            "name": d.name,
            "version": d.version,
            "purl": &r,
        });
        if let Some(c) = &d.checksum {
            comp["hashes"] = json!([{ "alg": "SHA-256", "content": c.to_ascii_lowercase() }]);
        }
        comps.push(comp);
    }

    let mut seed = format!("{name}|{version}");
    for d in &deps {
        seed.push('|');
        seed.push_str(&d.name);
        seed.push('@');
        seed.push_str(&d.version);
        if let Some(c) = &d.checksum {
            seed.push('#');
            seed.push_str(c);
        }
    }

    let doc = json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": format!("urn:uuid:{}", content_uuid(&seed)),
        "version": 1,
        "metadata": {
            "tools": {
                "components": [{
                    "type": "application",
                    "name": "lykaonc-cli",
                    "version": env!("CARGO_PKG_VERSION"),
                }]
            },
            "component": {
                "type": "application",
                "bom-ref": &root_ref,
                "name": name,
                "version": version,
                "purl": &root_ref,
            }
        },
        "components": comps,
        "dependencies": [{
            "ref": root_ref,
            "dependsOn": depends_on,
        }],
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}
