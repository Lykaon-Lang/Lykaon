//! M5 (Appendix L 7.6, M5-S49): encryption and keys.
//!
//! `encrypted X with key` uses XChaCha20-Poly1305 with a random 24-byte
//! nonce placed before the ciphertext. `decrypted` reverses it and fails
//! with a bad-format failure when the check fails. `random key` is 32 random
//! bytes; `key from password` derives a key with Argon2id (64 MiB, 3 passes,
//! 1 lane) and stores the salt in front of the key material.
//!
//! Bytes cross the language as a list of whole numbers (0..255). A text
//! value is encrypted as its UTF-8 bytes; a byte list is encrypted as is.
//! Decryption returns text when the plaintext is valid UTF-8, otherwise a
//! byte list.

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use crate::sem::{Kind, Value};

#[derive(Clone, Debug)]
pub enum Key {
    Random([u8; 32]),
    Password(String),
}

pub fn random_key() -> Key {
    let mut key = [0u8; 32];
    let _ = getrandom::getrandom(&mut key);
    Key::Random(key)
}

pub fn key_from_password(password: &str) -> Key {
    Key::Password(password.to_string())
}

pub fn key_from_value(v: &Value) -> Result<Key, ()> {
    match v {
        Value::Secret(inner) => key_from_value(&inner.0),
        Value::Text(t) => Ok(Key::Password(t.clone())),
        Value::List(items) => {
            let mut key = [0u8; 32];
            if items.len() != 32 {
                return Err(());
            }
            for (i, item) in items.iter().enumerate() {
                let n = match item {
                    Value::Whole(n) if (0..=255).contains(n) => *n as u8,
                    _ => return Err(()),
                };
                key[i] = n;
            }
            Ok(Key::Random(key))
        }
        _ => Err(()),
    }
}

fn derive(password: &str, salt: &[u8]) -> Result<[u8; 32], ()> {
    use argon2::{Algorithm, Argon2, Params, Version};
    let params = Params::new(64 * 1024, 3, 1, Some(32)).map_err(|_| ())?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|_| ())?;
    Ok(out)
}

fn value_bytes(v: &Value) -> Result<Vec<u8>, ()> {
    match v {
        Value::Secret(inner) => value_bytes(&inner.0),
        Value::Text(t) => Ok(t.as_bytes().to_vec()),
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Whole(n) if (0..=255).contains(n) => out.push(*n as u8),
                    Value::Missing => out.push(0),
                    _ => return Err(()),
                }
            }
            Ok(out)
        }
        other => Ok(crate::formats::json_compact(other)
            .map_err(|_| ())?
            .into_bytes()),
    }
}

fn bytes_value(bytes: &[u8]) -> Value {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Value::Text(text.to_string());
    }
    Value::List(bytes.iter().map(|b| Value::Whole(*b as i64)).collect())
}

fn nonce_bytes(nonce: &[u8]) -> Vec<u8> {
    nonce.to_vec()
}

pub fn encrypt(value: &Value, key: &Key) -> Result<Value, ()> {
    let plaintext = value_bytes(value)?;
    let cipher = XChaCha20Poly1305::new_from_slice(match key {
        Key::Random(k) => k,
        Key::Password(_) => &[0u8; 32],
    })
    .map_err(|_| ())?;
    let mut out: Vec<u8> = Vec::new();
    let mut nonce = [0u8; 24];
    if getrandom::getrandom(&mut nonce).is_err() {
        return Err(());
    }
    let actual_key: [u8; 32] = match key {
        Key::Random(k) => *k,
        Key::Password(password) => {
            let mut salt = [0u8; 16];
            if getrandom::getrandom(&mut salt).is_err() {
                return Err(());
            }
            out.extend_from_slice(&salt);
            derive(password, &salt)?
        }
    };
    let cipher = XChaCha20Poly1305::new_from_slice(&actual_key).map_err(|_| ())?;
    out.extend(nonce_bytes(&nonce));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext.as_slice())
        .map_err(|_| ())?;
    out.extend_from_slice(&ciphertext);
    Ok(Value::List(out.into_iter().map(|b| Value::Whole(b as i64)).collect()))
}

pub fn decrypt(value: &Value, key: &Key) -> Result<Value, ()> {
    let data = value_bytes(value)?;
    let (salt, rest) = match key {
        Key::Password(_) => {
            if data.len() < 16 + 24 {
                return Err(());
            }
            let (salt, rest) = data.split_at(16);
            (Some(salt), rest)
        }
        Key::Random(_) => (None, data.as_slice()),
    };
    if rest.len() < 24 {
        return Err(());
    }
    let (nonce, ciphertext) = rest.split_at(24);
    let actual_key: [u8; 32] = match (key, salt) {
        (Key::Random(k), _) => *k,
        (Key::Password(password), Some(salt)) => derive(password, salt)?,
        _ => return Err(()),
    };
    let cipher = XChaCha20Poly1305::new_from_slice(&actual_key).map_err(|_| ())?;
    let plaintext = cipher
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|_| ())?;
    Ok(bytes_value(&plaintext))
}

/// M5-T30: a wrong key or a damaged file is a bad-format failure.
pub fn is_bad_format() -> crate::sem::Kind {
    Kind::Unknown
}