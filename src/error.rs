//! The library's typed error. Callers can match on named variants; the binary
//! (`main.rs`) wraps this in `anyhow`, following the convention that libraries
//! use `thiserror` and only binaries reach for `anyhow`.

use thiserror::Error;

/// Boxed source error. gix surfaces a distinct concrete error type per
/// operation, so boxing preserves the source chain without this enum having to
/// enumerate (and track, across gix versions) every one of them.
type Source = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Result alias for the library's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Anything that can go wrong while gathering git statistics.
#[derive(Debug, Error)]
pub enum Error {
    /// No git repository could be discovered from the given path.
    #[error("could not open a git repository")]
    OpenRepository(#[source] Source),

    /// A revision (typically an endpoint of the range) could not be resolved.
    #[error("could not resolve revision {revision:?}")]
    ResolveRevision {
        revision: String,
        #[source]
        source: Source,
    },

    /// Walking the revision range failed.
    #[error("could not walk revision range {range:?}")]
    WalkRange {
        range: String,
        #[source]
        source: Source,
    },

    /// A commit object could not be read or decoded.
    #[error("could not read commit data")]
    ReadCommit(#[source] Source),

    /// A specific commit object could not be decoded, typically because its
    /// header is corrupt in a way git itself would never write.
    #[error("could not decode commit {id}")]
    DecodeCommit {
        id: String,
        #[source]
        source: Source,
    },

    /// Computing a commit's diff stats failed.
    #[error("could not compute diff stats")]
    DiffStats(#[source] Source),

    /// An `--author` pattern was not a valid regular expression.
    #[error("invalid author pattern")]
    AuthorPattern(#[from] regex::Error),

    /// A `--since`/`--until` value was not a recognized date.
    #[error("invalid date {input:?}: {message}")]
    InvalidDate { input: String, message: String },
}
