//! Lykaon file extensions (owner shortlist, 2026-11-26).
//!
//! Old names are normalized here so the compiler and tools stop using them.

pub const SOURCE: &str = "lykaon";
pub const PROJECT: &str = "Lykaon.toml";
pub const LOCK: &str = "lykaon.lock";
pub const LIBRARY_COLLECTION: &str = "shelf";
pub const PACKAGE: &str = "ship";
pub const BYTECODE: &str = "byte";
pub const GAME_ASSET: &str = "game";
pub const RESOURCE: &str = "resc";

/// Map an old extension word to the current one.
pub fn normalize(word: &str) -> &str {
    if word.eq_ignore_ascii_case("nest") {
        RESOURCE
    } else if word.eq_ignore_ascii_case("zip") {
        PACKAGE
    } else {
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_names_map_to_the_shortlist() {
        assert_eq!(normalize("nest"), "resc");
        assert_eq!(normalize("NEST"), "resc");
        assert_eq!(normalize("zip"), "ship");
        assert_eq!(normalize("lykaon"), "lykaon");
    }
}