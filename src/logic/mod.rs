//! Pure logic: filtering, aggregation, sorting, and rendering. No I/O. Its only gix use is
//! `gix::date`, which parses dates against an injected `now`, so every function here is directly
//! unit- and property-testable.

pub mod filter;
pub mod render;
pub mod table;
