//! `git-stats` aggregates per-author commit statistics from a git repository.
//!
//! The crate follows an A-Frame architecture: pure [`logic`] and gix-backed
//! [`repo`] infrastructure are peers, coordinated by the thin [`app`] layer.
//! Both sides communicate through the plain value objects in [`model`].

pub mod app;
pub mod cli;
pub mod error;
pub mod logic;
pub mod model;
pub mod repo;

pub use error::Error;
