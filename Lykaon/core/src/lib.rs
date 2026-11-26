//! lykaon-core: the typed tree, the value model and the interpreter.
//! Shared by the compiler (`lykaon`) and the runtime library (`lykaon-rt`)
//! that is linked into every built program.

pub mod alloc;
pub mod args;
pub mod chart;
pub mod font8x8;
#[cfg(feature = "crypto")]
pub mod crypto;
pub mod data;
pub mod db;
pub mod formats;
pub mod model;
pub mod interp;
pub mod layout;
pub mod migrate;
pub mod mir;
pub mod net;
pub mod png;
pub mod sandbox;
pub mod sem;
pub mod signals;
pub mod time4;
pub mod trace;
