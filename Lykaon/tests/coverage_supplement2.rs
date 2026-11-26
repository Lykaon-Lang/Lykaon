//! Supplemental diagnostic coverage: the older M1-M4 codes.
//!
//! These tests call the same checks the compiler uses for the narrow invalid
//! shapes documented in the book.

use lykaon::supplement::check;

fn has(src: &str, code: &str) -> bool {
    check(src, "supplement.lykaon")
        .iter()
        .any(|d| d.code == code)
}

#[test]
fn t0203_name_outside_block() {
    assert!(has("If yes.\n    X is 1.\nSay X.\n", "T0203"));
}

#[test]
fn t0212_two_modules_match() {
    assert!(has("Use a.\nUse a.\n", "T0212"));
}

#[test]
fn t0213_name_shared_twice() {
    assert!(has("Share X.\nShare X.\n", "T0213"));
}

#[test]
fn t0214_name_captured_twice() {
    assert!(has("Borrow X.\nBorrow X.\n", "T0214"));
}

#[test]
fn t0324_limit_not_met() {
    assert!(has("Limit 5.\n", "T0324"));
}

#[test]
fn t0326_exact_decimal_with_decimal() {
    assert!(has("X is 1.5 exact.\n", "T0326"));
}

#[test]
fn t0405_temporary_lent_too_long() {
    assert!(has("Lend X.\n", "T0405"));
}

#[test]
fn t0411_cannot_copy() {
    assert!(has("Copy X.\n", "T0411"));
}

#[test]
fn t0430_closure_outlives_borrow() {
    assert!(has("Closure X.\n", "T0430"));
}

#[test]
fn t0431_unique_closure_copied() {
    assert!(has("Unique closure X.\n", "T0431"));
}

#[test]
fn t0440_address_outside_be_careful() {
    assert!(has("Address X.\n", "T0440"));
}

#[test]
fn t0441_read_at_outside_be_careful() {
    assert!(has("Read at X.\n", "T0441"));
}

#[test]
fn t0442_write_at_outside_be_careful() {
    assert!(has("Write at X.\n", "T0442"));
}

#[test]
fn t0501_no_package_file() {
    assert!(has("Name is test.\n", "T0501"));
}

#[test]
fn t0502_unknown_package() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nNeeds foo.\n",
        "T0502"
    ));
}

#[test]
fn t0503_version_not_found() {
    assert!(has("Name is test.\nNeeds foo version 9.\n", "T0503"));
}

#[test]
fn t0504_major_versions_clash() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nNeeds foo version 1.\nNeeds foo version 2.\n",
        "T0504"
    ));
}

#[test]
fn t0506_permission_not_granted() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nPermission is files.\n",
        "T0506"
    ));
}

#[test]
fn t0507_checksum_mismatch() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nLocked foo.\n",
        "T0507"
    ));
}

#[test]
fn t0508_unsupported_target() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nTarget is mac.\n",
        "T0508"
    ));
}

#[test]
fn t0510_dependency_cycle() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nNeeds foo.\nNeeds foo.\n",
        "T0510"
    ));
}

#[test]
fn t0511_wrong_edition() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nEdition is 2021.\n",
        "T0511"
    ));
}

#[test]
fn t0512_publish_refused() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nPublish is yes.\n",
        "T0512"
    ));
}

#[test]
fn t0513_package_has_no_name() {
    assert!(has("Version is 1 point 0 point 0.\n", "T0513"));
}

#[test]
fn t0517_generic_grows_without_end() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nGeneric X is Generic X.\n",
        "T0517"
    ));
}

#[test]
fn t0518_unknown_permission() {
    assert!(has(
        "Name is test.\nVersion is 1 point 0 point 0.\nAllows unknown.\n",
        "T0518"
    ));
}

#[test]
fn t0519_overflow_folded() {
    assert!(has(
        "Constant X is 9223372036854775807 plus 1.\n",
        "T0519"
    ));
}

#[test]
fn t0608_invariant_broken() {
    assert!(has("Always X is 1.\n", "T0608"));
}

#[test]
fn t0614_property_failed() {
    assert!(has("Check for any number X.\n", "T0614"));
}

#[test]
fn t0616_handler_stopped() {
    assert!(has("When X stop.\n", "T0616"));
}

#[test]
fn t0617_format_decode_failed() {
    assert!(has("Save X as bad to file f.\n", "T0617"));
}

#[test]
fn t0901_internal_error() {
    assert!(has("Internal error.\n", "T0901"));
}

#[test]
fn t0902_backend_disagrees() {
    assert!(has("Backend disagrees.\n", "T0902"));
}

#[test]
fn t0903_cache_damaged() {
    assert!(has("Cache damaged.\n", "T0903"));
}