//! The interpreter, moved into lykaon-core (M3): the same tree-walking
//! evaluator that defines what every program means (Ch.33: "the reference
//! behaviour is defined by the language rules and the interpreter"). The
//! runtime library linked into built programs calls this evaluator for the
//! expression nodes and statement effects the backend does not lower, which
//! is what makes the differential guarantee (native output equals
//! interpreter output, byte for byte) hold by construction.

pub use lykaon_core::interp::{
    ExitReason, RunError, run, run_budgeted, run_budgeted_streams,
    run_budgeted_streams_metered, run_budgeted_streams_traced, run_test, split_streams,
};
