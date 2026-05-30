//! Pure logic: filtering, aggregation, sorting, and rendering. No I/O and no
//! gix dependency, so every function here is directly unit- and property-testable.

pub mod aggregate;
pub mod filter;
pub mod render;
pub mod sort;
