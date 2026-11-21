/**
 * Tree-sitter grammar for Lykaon (M1-M4).
 *
 * Lykaon is indentation-based and sentence-oriented; for syntax highlighting a
 * flat token stream is enough (editors only need to colour keywords, numbers,
 * comments and operators). The grammar is highlighting-focused: it does not try
 * to model the block structure.
 */

const KEYWORDS = [
    "Say", "Set", "Ask", "If", "Otherwise", "Repeat", "For", "each", "in",
    "from", "to", "while", "forever", "Stop", "Skip", "Give", "Hand", "back",
    "To", "A", "has", "is", "Changeable", "Constant", "Note", "About",
    "Test", "Check", "Try", "When", "Look", "Be", "careful", "Do", "these",
    "together", "Start", "task", "Wait", "Sleep", "Send", "Receive", "Run",
    "Exit", "Read", "Write", "Append", "Replace", "Make", "Remove", "Move",
    "Duplicate", "Zip", "Unzip", "Gzip", "Save", "Fetch", "Post", "Respond",
    "Serve", "Page", "Log", "Takes", "Option", "Plot", "Predict", "Open",
    "Limit", "Requires", "Ensures", "Always", "Apply", "Combine", "Use",
    "Share", "Anything", "Tagged", "Load", "Navigate", "Show", "Play", "Hide",
    "plus", "minus", "times", "divided", "by", "whole", "and", "or", "not",
    "yes", "no", "missing", "nothing", "with", "as", "of", "where", "that",
    "fails", "can", "fail", "copy", "share", "weak", "give", "lend", "change",
];

module.exports = grammar({
    name: "lykaon",

    extras: $ => [/\s/],

    word: $ => $.name,

    rules: {
        program: $ => repeat($._item),

        _item: $ => choice(
            $.comment,
            $.keyword,
            $.number,
            $.operator,
            $.name,
            $.punctuation,
        ),

        comment: $ => seq(
            choice("Note", "About"),
            /[^\n]*/,
        ),

        keyword: $ => choice(...KEYWORDS),

        number: $ => /\d+(\.\d+)?/,

        name: $ => /[A-Za-z_][A-Za-z0-9_]*/,

        operator: $ => choice(
            "plus", "minus", "times", "divided by", "whole divided by",
            "is", "is not", "is more than", "is less than", "is at least",
            "is at most",
        ),

        punctuation: $ => choice(".", ",", "(", ")", "[", "]"),
    },
});