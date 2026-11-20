# Lykaon — M1–M4 grammar (normative extract)

This is the grammar implemented by the current M1–M4 compiler and interpreter.
It is extracted from Appendix B of *Lykaon: Programming in Plain English v0.1*
and covers rules G1–G227 (M1–M4). M5+ rules (tables, AI, databases, sandbox,
wasm) are outside this reference; see Appendix L of the book.

Notation: `=` is the rule, words in quotes are matched case-insensitively,
`optional (...)`, `repeated (...)`, `one of` are the book's notation. A rule
marked *pattern* is a row of the pattern table (Appendix C).

  G1 WORD = letter, repeated (letter or digit)
 G2 NUMBER = digits, optional ("." digits)
 G3 NAME = one or more WORD that are not hard words; the longest defined name wins
 G4 TEXT = the raw characters of the rest of the line, up to the final period
 G5 TEXT_WITH_NAMES = TEXT in which each word equal to a defined name stands for its value
 G6 TEXT_LINE = the raw characters of one line under "Say lines."
 G7 NEWLINE = the end of a line
 G8 INDENT = 4 more spaces at the start of a line
 G9 DEDENT = a return to fewer spaces at the start of a line
 G10 sentence_end = ".", NEWLINE
 G11 program = repeated top_item
 G12 top_item = one of note, about_block, function_def, record_def, enum_def,
interface_def, impl_def, use_stmt, share_stmt, const_def, test_block, statement,
load_stmt, tag_stmt, use_allocator, register_def, memory_limit, kernel_def, gpu_block,
screen_block, style_block, theme_use, theme_def, describe_stmt, c_library, share_for_c,
plugin_load, takes_stmt, option_stmt
 G13 statement = one of simple_stmt, block_stmt
 G14 simple_stmt = one of define, changeable, set, say, ask, load, stop, skip, give_back,
hand_back, check, add_stmt, remove_stmt, remove_first, put_stmt, fail_stmt, pass_up,
stop_program, pretend_stmt, recover_line, pattern_stmt, load_stmt, apply_stmt,
combine_stmt, send_stmt, receive_stmt, wait_stmt, sleep_stmt, atomic_add, workers_stmt,
stop_task, run_stmt, exit_stmt, read_file, read_bytes, write_file, append_file,
replace_file, make_folder, remove_file, move_file, duplicate_file, temp_file, zip_stmt,
read_format, read_text_format, save_format, seed_stmt, fetch_stmt, post_stmt,
respond_stmt, log_stmt, log_level_stmt, takes_stmt, option_stmt, plot_stmt, predict_stmt,
predict_gpu, save_model, load_model, assistant_stmt, save_table, open_db, db_load, db_add,
db_remove, db_set, limit_stmt, navigation, show_stmt, describe_stmt, play_sound,
stop_sound, move_picture, hide_picture, intrinsic_stmt, write_register, plugin_load,
requires_stmt, ensures_stmt
 G15 block_stmt = one of if_block, if_line, otherwise_block, otherwise_line, repeat_block,
repeat_line, for_each_in, for_each_range, try_block, look_block, check_fails, check_any,
careful_block, together_block, task_block, pattern_block, after_block, every_block,


===== PAGE 202 =====
Lykaon: Programming in Plain English  -  Language version 0.1
202
stream_loop, spread_block, signal_block, file_watch, serve_block, when_someone,
live_block, layout_block, event_block, receive_event, responsive_block, frame_block,
key_event, pointer_event, app_event, interrupt_block, transaction_block, gpu_block
 G16 block = INDENT, repeated statement, DEDENT
 G17 pattern_stmt = a row of the pattern table that starts with an action word and ends
with sentence_end
 G18 pattern_block = a row of the pattern table that starts with an action word and ends
with a block
 G19 simple_stmt_no_period = a simple_stmt written without its sentence_end
 G20 one_line_action = one of "say", "set", "stop", "skip", "give", "hand", "fail",
"exit", "add", "remove", "put", "send"
 G21 note = "Note", TEXT, sentence_end
 G22 about_block = "About", TEXT, sentence_end, optional (INDENT, repeated example_block,
DEDENT)
 G23 define = NAME, "is", expr, sentence_end
 G24 changeable = "Changeable", NAME, "is", expr, sentence_end
 G25 const_def = "Constant", NAME, "is", literal, sentence_end
 G26 set = "Set", place, "to", expr, sentence_end
 G27 say = one of say_question, say_lines, say_value, say_text (the first form that fits
the whole line is used), say_data, group_say, group_all, row_say, say_model
 G28 say_question = "Say", "if", cond, sentence_end
 G29 say_lines = "Say", "lines", sentence_end, INDENT, repeated TEXT_LINE, DEDENT
 G30 say_value = "Say", expr, optional fmt, sentence_end
 G31 say_text = "Say", optional (one of "literally", ("without" "newline")),
TEXT_WITH_NAMES, optional fmt, sentence_end
 G32 fmt = one of ("to" NUMBER "places"), ("width" NUMBER)
 G33 ask = "Ask", NAME, optional ("as" type), sentence_end
 G34 stop = "Stop", sentence_end
 G35 skip = "Skip", sentence_end
 G36 give_back = "Give", "back", expr, sentence_end
 G37 hand_back = "Hand", "back", expr, sentence_end
 G38 if_block = "If", cond, sentence_end, block
 G39 if_line = "If", cond, simple_stmt_no_period, ".", NEWLINE (the question ends at the
first one_line_action word)
 G40 otherwise_block = "Otherwise", optional ("if" cond), sentence_end, block
 G41 otherwise_line = "Otherwise", optional ("if" cond), simple_stmt_no_period, ".",
NEWLINE
 G42 repeat_block = "Repeat", one of (NUMBER "times"), ("while" cond), "forever",
sentence_end, block
 G43 repeat_line = "Repeat", NUMBER, "times", simple_stmt_no_period, ".", NEWLINE
 G44 for_each_in = "For", "each", names, "in", expr, sentence_end, block
 G45 for_each_range = "For", "each", NAME, "from", expr, "to", expr, sentence_end, block


===== PAGE 203 =====
Lykaon: Programming in Plain English  -  Language version 0.1
203
 G46 try_block = "Try", optional failing_stmt_no_period, sentence_end, block, "If",
"that", "fails", optional ("with" kind), sentence_end, block
 G47 look_block = "Look", "at", operand, sentence_end, repeated when_block
 G48 careful_block = "Be", "careful", sentence_end, block
 G49 together_block = "Do", "these", "together", sentence_end, block
 G50 task_block = "Start", "task", NAME, sentence_end, block
 G51 function_def = "To", function_name, optional param, repeated ("with" param), optional
("can" "fail"), optional ("on" "the" "graphics" "card"), one of ("give" expr
sentence_end), (sentence_end, block)
 G52 param = optional (one of "give", ("lend" "to" "change")), NAME, optional ("as"
param_type), optional ("defaulting" "to" operand), optional ("lasting" "as" "long" "as"
NAME)
 G53 record_def = "A", NAME, "has", field, repeated ("and" field), optional (layout_phrase
repeated ("and" layout_phrase)), sentence_end, optional (INDENT repeated always_stmt
DEDENT)
 G54 field = NAME, "as", type
 G55 enum_def = "A", NAME, "is", variant, repeated ("or" variant), sentence_end
 G56 variant = NAME, optional ("with" field)
 G57 interface_def = "Anything", "that", "can", NAME, "must", "have", NAME, repeated
("and" NAME), sentence_end, optional default_impl
 G58 impl_def = one of function_impl, meaning_impl
 G59 use_stmt = "Use", NAME, optional ("from" NAME), sentence_end
 G60 share_stmt = "Share", NAME, sentence_end
 G61 test_block = "Test", TEXT, sentence_end, block
 G62 check = "Check", "that", cond, sentence_end
 G63 check_fails = "Check", "that", "this", "fails", sentence_end, block
 G64 check_any = "Check", "for", "any", type, NAME, sentence_end, block
 G65 example_block = "Example", TEXT, sentence_end, block
 G66 pretend_stmt = "Pretend", NAME, "gives", operand, sentence_end
 G67 cond = one of comparison, (cond "and" comparison), (cond "or" comparison), ("not"
cond) (and and or are never mixed in one sentence)
 G68 comparison = one of (operand cmp operand), (operand "is" "missing"), (operand "is"
"there"), (operand "is" "empty"), (operand "has" operand), question_pattern,
file_question, touches_question, collection_question
 G69 question_pattern = a row of the pattern table that gives yes or no
 G70 cmp = one of "is", ("is" "not"), ("is" "more" "than"), ("is" "less" "than"), ("is"
"at" "least"), ("is" "at" "most"), ("is" "close" "to"), ("is" "between" operand "and")
 G71 expr = one of operand, (operand BINOP operand), (operand postfix), call, builtin,
list_value, inline_list, array_value, map_value, set_value, queue_value, pair_value,
record_make, variant_value, copy_expr, give_expr, where_expr, sequence_read, vector_value,
vector_read, encrypt_expr, decrypt_expr, key_expr, date_value, moment_value, time_arith,
random_read, hash_read, secret_value, reveal_read, format_text, environment_read,
read_register, request_read, collection_read, set_read, pair_read, text_read
 G72 BINOP = one of "plus", "minus", "times", ("divided" "by"), ("whole" "divided" "by")


===== PAGE 204 =====
Lykaon: Programming in Plain English  -  Language version 0.1
204
 G73 postfix = one of "sorted", ("sorted" "descending"), "reversed", "unique", "trimmed",
"backwards", "rounded", ("rounded" "to" "even"), ("fallback" operand), ("as" type) (only
after a name, a number, a text word or a list of items)
 G74 operand = one of literal, NAME, property, pronoun
 G75 property = one of (NAME "of" operand), ("item" NUMBER "of" operand)
 G76 pronoun = one of "it", ("its" NAME), "them"
 G77 literal = one of NUMBER, ("minus" NUMBER), "yes", "no", "missing", "nothing" ("minus"
is a sign only where an operand begins)
 G78 function_name = a single WORD that is not a hard word
 G79 call = function_name, optional operand, repeated ("with" operand)
 G80 list_value = one of ("list" "of" operand repeated ("and" operand)), ("empty" "list"
"of" type)
 G81 inline_list = operand, "and", operand, repeated ("and" operand) (only where a pattern
takes a list, and as the whole of a Say)
 G82 array_value = "array", "of", NUMBER, type
 G83 map_value = "map", "of", type, "to", type
 G84 set_value = "set", "of", type
 G85 queue_value = "queue", "of", type
 G86 pair_value = "pair", "of", operand, "and", operand
 G87 record_make = NAME, "with", part_value, repeated ("and" part_value) (NAME is a record
kind; the article before it is removed)
 G88 part_value = NAME, operand
 G89 variant_value = variant_name, optional ("with" part_value repeated ("and"
part_value))
 G90 variant_name = a variant of a declared enum, defined when the enum is declared
 G91 copy_expr = "copy", "of", operand
 G92 give_expr = "give", operand
 G93 add_stmt = "Add", operand, "to", NAME, sentence_end
 G94 remove_stmt = "Remove", operand, "from", NAME, sentence_end
 G95 remove_first = "Remove", "first", "from", NAME, sentence_end
 G96 put_stmt = "Put", operand, "in", NAME, optional ("under" operand), sentence_end
 G97 failing_stmt = a call to a function that can fail, or a row of the pattern table
marked can fail
 G98 failing_stmt_no_period = a failing_stmt written without its sentence_end
 G99 recover_line = failing_stmt, sentence_end, "If", "that", "fails", "use", operand,
".", NEWLINE
 G100 fail_stmt = "Fail", "with", TEXT, sentence_end
 G101 pass_up = "Pass", "failure", "up", sentence_end
 G102 stop_program = "Stop", "program", "with", TEXT, sentence_end
 G103 text_pattern = repeated pattern_part
 G104 pattern_part = one of "digits", "letters", "spaces", "anything", TEXT_WORD, ("one"
"or" "more" pattern_part), ("optional" pattern_part), ("starts" "with" TEXT_WORD), ("ends"
"with" TEXT_WORD)


===== PAGE 205 =====
Lykaon: Programming in Plain English  -  Language version 0.1
205
 G105 builtin = a row of the pattern table that gives a value
 G106 type = one of base_type, ("list of" type), ("map of" type "to" type), ("set of"
type), ("queue of" type), ("maybe" type), type_extra, ("pair of" type "and" type), ("array
of" NUMBER type) (a base kind word may end in s inside a type)
 G107 base_type = one of "number", "whole number", "decimal", "exact decimal", "big
number", "text", ("yes" "or" "no"), "letter", "bytes", NAME
 G108 names = NAME, repeated ("and" NAME)
 G109 kind = one of ("not" "found"), ("no" "permission"), ("bad" "format"), ("timed"
"out"), "other"
 G110 when_block = "When", variant_name, repeated NAME, sentence_end, block
 G111 place = one of ("path" NAME repeated ("and" NAME)), NAME
 G112 param_type = one of type, limited_type
 G113 limited_type = "anything", "that", "can", NAME, repeated ("and" NAME)
 G114 function_kind = "function", "from", type, "to", type
 G115 dynamic_kind = "any", agent_noun
 G116 default_impl = "By", "default", NAME, "says", one of "nothing", TEXT, sentence_end
 G117 function_impl = "A", NAME, "can", NAME, sentence_end, INDENT, repeated function_def,
DEDENT
 G118 meaning_impl = "A", NAME, "can", one of "add", "compare", "copy", ("clean" "up"),
("be" "saved"), sentence_end, block
 G119 apply_stmt = "Apply", function_ref, "to", "each", "of", operand, optional
gpu_clause, sentence_end
 G120 combine_stmt = "Combine", operand, "with", function_ref, sentence_end
 G121 function_ref = one of function_name, operator_word
 G122 operator_word = one of "plus", "minus", "times", "biggest", "smallest"
 G123 where_expr = operand, "where", cond
 G124 sequence_read = one of ("first" NUMBER "fibonacci"), ("primes" "to" NUMBER),
("first" operand "of" operand)
 G125 give_back_typed = one of ("Give" "back" expr "as" type sentence_end)
 G126 requires_stmt = "Requires", optional "checked", cond, sentence_end
 G127 ensures_stmt = "Ensures", optional "checked", cond, sentence_end
 G128 always_stmt = "Always", cond, sentence_end
 G129 tag_stmt = "Tagged", NAME, sentence_end
 G130 contract_position = a block function begins with zero or more requires_stmt and
ensures_stmt before any other sentence
 G131 definition_with_tag = repeated tag_stmt, one of function_def, record_def, enum_def,
interface_def, test_block
 G132 package_file = repeated package_sentence
 G133 package_sentence = one of name_stmt, version_stmt, edition_stmt, entry_stmt,
needs_stmt, allows_stmt, page_stmt
 G134 name_stmt = "Name", "is", NAME, sentence_end
 G135 version_stmt = "Version", "is", version, sentence_end
 G136 version = NUMBER, "point", NUMBER, "point", NUMBER


===== PAGE 206 =====
Lykaon: Programming in Plain English  -  Language version 0.1
206
 G137 edition_stmt = "Edition", "is", NUMBER, sentence_end
 G138 entry_stmt = "Entry", "is", NAME, sentence_end
 G139 needs_stmt = "Needs", NAME, one of ("version" version_req), ("from" "folder" NAME
repeated ("in" "folder" NAME)), ("from" "file" NAME "with" "extension" "pole"),
sentence_end
 G140 version_req = NUMBER, optional ("point" NUMBER, optional ("point" NUMBER))
 G141 allows_stmt = "Allows", permission, repeated ("and" permission), sentence_end
 G142 permission = one of ("files" optional ("in" folder_ref)), ("network" optional ("to"
"host" host_words)), ("environment" optional ("variable" names)), "processes"
 G143 lock_file = repeated lock_sentence
 G144 lock_sentence = "Locked", NAME, "version", version, one of ("from" "folder"
path_words), ("checksum" HEX), sentence_end
 G145 type_extra = one of function_kind, dynamic_kind
 G146 contract_in_one_line = not allowed; a one-line function has no contract
 G147 wait_stmt = "Wait", "for", NAME, sentence_end
 G148 sleep_stmt = "Sleep", "for", duration_value, sentence_end
 G149 duration_value = NUMBER, time_unit
 G150 time_unit = one of "milliseconds", "seconds", "minutes", "hours", "days", "weeks"
 G151 channel_value = "channel", "of", type, optional ("holding" NUMBER)
 G152 guarded_value = "guarded", operand
 G153 atomic_value = "atomic", "number"
 G154 counter_value = "counter"
 G155 send_stmt = "Send", operand, "to", NAME, sentence_end
 G156 receive_stmt = "Receive", "from", NAME, sentence_end
 G157 atomic_add = "Add", operand, "to", NAME, "atomically", sentence_end
 G158 after_block = "After", duration_value, sentence_end, block
 G159 every_block = "Every", duration_value, sentence_end, block
 G160 stream_loop = "For", "each", NAME, "as", "they", "arrive", "from", NAME,
sentence_end, block
 G161 workers_stmt = "Use", NUMBER, "workers", sentence_end
 G162 spread_block = "Spread", "each", "of", operand, "over", "workers", sentence_end,
block
 G163 stop_task = "Stop", "task", NAME, sentence_end
 G164 run_stmt = "Run", NAME, optional ("with" operand, repeated ("and" operand)),
sentence_end
 G165 exit_stmt = "Exit", "with", NUMBER, sentence_end
 G166 signal_block = "When", "signal", signal_name, "arrives", sentence_end, block
 G167 signal_name = one of "stop", "interrupt", "hangup"
 G168 environment_read = "Environment", names
 G169 takes_stmt = "Takes", NAME, "as", type, sentence_end
 G170 option_stmt = "Option", NAME, "is", type, optional ("defaulting" "to" operand),
sentence_end
 G171 input_read = one of ("Read" "input"), ("For" "each" "line" "in" "input")


===== PAGE 207 =====
Lykaon: Programming in Plain English  -  Language version 0.1
207
 G172 file_ref = one of (file_name_words, optional extension, repeated folder_step,
optional root), ("file" "named" TEXT)
 G173 file_name_words = "file", NAME
 G174 extension = "with", "extension", NAME
 G175 folder_step = "in", "folder", NAME
 G176 root = one of ("in" "parent" "folder"), ("in" "home" "folder"), ("in" "current"
"folder"), ("in" "temporary" "folder"), ("in" "root" "folder"), ("on" "drive" NAME)
 G177 folder_ref = "folder", NAME, repeated folder_step, optional root
 G178 read_file = "Read", file_ref, sentence_end
 G179 read_bytes = "Read", "bytes", "of", file_ref, sentence_end
 G180 write_file = "Write", operand, "to", file_ref, sentence_end
 G181 append_file = "Append", operand, "to", file_ref, sentence_end
 G182 replace_file = "Replace", file_ref, "with", operand, sentence_end
 G183 for_each_line = "For", "each", NAME, "in", file_ref, sentence_end, block
 G184 make_folder = "Make", folder_ref, sentence_end
 G185 remove_file = "Remove", one of file_ref, folder_ref, sentence_end
 G186 move_file = "Move", file_ref, "to", file_ref, sentence_end
 G187 duplicate_file = "Duplicate", file_ref, "as", file_ref, sentence_end
 G188 list_folder = "List", "files", "in", folder_ref
 G189 file_question = file_ref, "exists"
 G190 temp_file = "Make", "temporary", "file", NAME, sentence_end
 G191 file_watch = "When", file_ref, "changes", sentence_end, block
 G192 zip_stmt = one of ("Zip" folder_ref "to" file_ref), ("Unzip" file_ref "to"
folder_ref), ("Gzip" file_ref "to" file_ref)
 G193 read_format = "Read", format_word, file_ref, "as", type, sentence_end
 G194 read_text_format = "Read", format_word, "from", operand, "as", type, sentence_end
 G195 save_format = "Save", operand, "as", format_word, "to", file_ref, sentence_end
 G196 format_text = format_word, "of", operand
 G197 format_word = one of "json", "toml", "yaml", "xml", "csv"
 G198 date_value = one of ("date" NUMBER "and" NUMBER "and" NUMBER), "today"
 G199 moment_value = one of "now", ("moment" "of" operand "at" NUMBER "and" NUMBER),
(operand "in" "zone" names)
 G200 time_arith = one of (NUMBER time_unit "after" operand), (NUMBER time_unit "before"
operand), ("days" "between" operand "and" operand), ("seconds" "between" operand "and"
operand)
 G201 random_read = one of ("random" "number" "from" operand "to" operand), ("random"
"decimal"), ("random" "item" "of" operand), (operand "shuffled")
 G202 seed_stmt = "Use", "seed", NUMBER, sentence_end
 G203 hash_read = "hash", "of", operand
 G204 secret_value = "secret", operand
 G205 reveal_read = "reveal", operand
 G206 address = optional scheme, host_words, optional port, optional place


===== PAGE 208 =====
Lykaon: Programming in Plain English  -  Language version 0.1
208
 G207 scheme = one of "secure", "plain"
 G208 host_words = NAME, repeated ("dot" NAME)
 G209 port = "on", "port", NUMBER
 G210 fetch_stmt = "Fetch", address, repeated fetch_clause, sentence_end
 G211 fetch_clause = one of ("using" method_word), ("with" "data" operand), ("with"
"header" NAME operand), ("with" "query" NAME operand)
 G212 method_word = one of "get", "post", "put", "delete", "patch"
 G213 post_stmt = "Post", operand, "to", address, sentence_end
 G214 serve_block = "Serve", optional "securely", "on", "port", NUMBER, optional
tls_clause, sentence_end, block
 G215 tls_clause = "using", "certificate", file_ref, "and", "key", file_ref
 G216 when_someone = "When", "someone", http_word, page_name, sentence_end, block
 G217 http_word = one of "gets", "posts", "puts", "deletes", "patches"
 G218 live_block = "When", "someone", "opens", "live", page_name, sentence_end, block
 G219 respond_stmt = "Respond", "with", one of TEXT, ("json" operand), ("status" NUMBER
optional ("and" TEXT)), ("template" NAME), ("literally" TEXT), sentence_end
 G220 request_read = one of ("method" "of" "request"), ("page" "of" "request"), ("text"
"of" "request"), ("data" "of" "request"), ("header" NAME "of" "request"), ("query" NAME
"of" "request"), ("parameter" NAME "of" "request")
 G221 page_stmt = "Page", names, "is", path_segments, sentence_end
 G222 path_segments = one of "nothing", (segment repeated ("and" segment))
 G223 segment = one of NAME, ("anything" "called" NAME)
 G224 log_stmt = "Log", level, TEXT, sentence_end
 G225 level = one of "debug", "info", "warning", "error"
 G226 log_level_stmt = "Use", "log", "level", level, sentence_end
 G227 needs_git = "Needs", NAME, "from", "git", address, optional ("at", "version",
version), sentence_end

## Semantic rules

The grammar above is the syntax. The meaning is defined by the M1–M4
milestone files and appendices:

- M1 — syntax and interpreter: `Prompt/M1.txt`, Appendix B/C.
- M2 — data, structure, ownership: `Prompt/M2.txt`, Appendix I.
- M3 — native code and abstractions: `Prompt/M3.txt`, Appendix J.
- M4 — servers and concurrency: `Prompt/M4.txt`, Appendix K.

Every implemented rule is covered by the conformance suites under
`Lykaon/tests/conformance/` and by the native differential tests.

## Error codes

The M1–M4 diagnostics are listed in Appendix D of the book and in
`docs/Lykaon_Language_Reference.md`; the Where/Why/Fix wording is the one the
compiler emits.
