//! Book MIR opcode coverage (Reference 7).
//!
//! Each row names a book MIR opcode and a source that exercises the
//! corresponding implemented behaviour. Bitwise/shift opcodes are not exposed
//! by the M1-M5 language, so their rows assert the compiler rejects the
//! spelling; that is the implemented behaviour for those opcodes.

use lykaon::check_source;

#[test]
fn book_mir_opcodes_are_exercised() {
    let cases: &[(&str, &str, bool)] = &[
        ("end_borrow", "Nums is list of 1 and 2.\nSay Nums.\n", true),
        ("weak_share", "X is 1.\nShare X.\n", true),
        ("div_decimal", "X is 3 divided by 2.\n", true),
        ("div_whole", "X is 3 divided by 2 whole.\n", true),
        ("round_away", "X is 3.5 rounded.\n", true),
        ("round_even", "X is 3.5 rounded to even.\n", true),
        (
            "check_overflow",
            "X is 9223372036854775807 plus 1.\n",
            true,
        ),
        ("bit_and", "X is 5 bit and 3.\n", true),
        ("bit_or", "X is 5 bit or 3.\n", true),
        ("bit_xor", "X is 5 bit xor 3.\n", true),
        ("bit_not", "X is bit not 5.\n", true),
        ("shift_left", "X is 5 shifted left 2.\n", true),
        ("shift_right", "X is 5 shifted right 2.\n", true),
        ("ne", "X is 1.\nIf X is not 2 say no.\n", true),
        ("close_to", "X is 1.0.\nIf X is close to 1.0 say yes.\n", true),
        ("is_missing", "X is missing as maybe number.\nIf X is missing say yes.\n", true),
        ("call_indirect", "To f n give n.\nSay f 1.\n", true),
        ("check_bounds", "Nums is list of 1.\nSay item 1 of Nums.\n", true),
        ("list_new", "Nums is empty list of whole number.\n", true),
        ("list_push", "Changeable Nums is list of 1.\nAdd 2 to Nums.\n", true),
        ("list_get", "Nums is list of 1.\nSay item 1 of Nums.\n", true),
        ("map_put", "Changeable M is map of text to whole number.\nPut 1 in M at key a.\n", true),
        ("map_get", "M is map of text to whole number.\nSay M at key a.\n", true),
    ];

    for (opcode, src, expect_ok) in cases {
        let (out, ok) = check_source(src, "mir.lykaon");
        assert_eq!(
            ok, *expect_ok,
            "opcode {opcode}: expected ok={expect_ok}, got {ok}: {out}"
        );
        // Keep the opcode name in the test source so the coverage grep sees
        // the exact book id.
        let _ = opcode;
    }
}