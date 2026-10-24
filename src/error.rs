//! The library's typed error. Callers can match on named variants; the binary (`main.rs`) wraps
//! this in `anyhow`, following the convention that libraries use `thiserror` and only binaries
//! reach for `anyhow`.

use thiserror::Error;

/// Boxed source error. gix surfaces a distinct concrete error type per operation, so boxing
/// preserves the source chain without this enum having to enumerate (and track, across gix
/// versions) every one of them.
pub(crate) type Source = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Result alias for the library's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Anything that can go wrong while gathering git statistics.
#[derive(Debug, Error)]
pub enum Error {
    /// No git repository could be discovered from the given path.
    #[error("could not open a git repository")]
    OpenRepository(#[source] Source),

    /// jj was requested, but no directory at or above the path holds `.jj`.
    #[error("no jj workspace found at or above {path:?}")]
    NoJjWorkspace { path: std::path::PathBuf },

    /// The `jj` binary could not be started, typically because it is not installed.
    #[error(
        "could not run `jj`; is it installed and on PATH? (`--vcs git` reads the git repository instead)"
    )]
    SpawnJj(#[source] std::io::Error),

    /// A `jj` command exited with an error. jj prints its own diagnostics to stderr, so this
    /// carries only the command and its exit status.
    #[error("`jj {command}` failed ({status})")]
    Jj {
        command: &'static str,
        status: std::process::ExitStatus,
    },

    /// A `jj` command printed output that is not what was requested.
    #[error("unexpected output from `jj {command}`: {output:?}")]
    JjOutput {
        command: &'static str,
        output: String,
    },

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

    /// A commit in the range could not be read, decoded, or diffed, for example because an object
    /// it needs is missing or corrupt.
    #[error("could not read commit {id}")]
    ReadCommit {
        id: String,
        #[source]
        source: Source,
    },

    /// The diff setup could not read the index or the `.gitattributes` it needs.
    #[error("could not read the attributes for diffs")]
    DiffSetup(#[source] Source),

    /// A file's contents in a commit could not be read to count its lines.
    #[error("could not read {path:?} in commit {commit}")]
    ReadBlob {
        commit: String,
        path: String,
        #[source]
        source: Source,
    },

    /// A read failed in a partial clone, which fetches objects only on demand. gitoxide does not
    /// fetch them, so a missing object is the likely cause.
    #[error(
        "this partial clone lacks objects the stats need; fetch them first, for example with `git log --numstat` over the same range"
    )]
    PartialClone(#[source] Box<Error>),

    /// An `--author` pattern was not a valid regular expression.
    #[error("invalid author pattern")]
    AuthorPattern(#[from] regex::Error),

    /// A `--since`/`--until` value was not a recognized date.
    #[error("invalid date {input:?}: {message}")]
    InvalidDate { input: String, message: String },
}
