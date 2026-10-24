//! Value objects: plain data passed between the pure logic and the gix infrastructure. Nothing here
//! performs I/O or depends on gix.

use clap::ValueEnum;

/// A commit author, resolved through `.mailmap` to match git's `%aN`/`%aE`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Author {
    pub name: String,
    pub email: String,
}

/// A single git trailer such as `Reviewed-by: Ada <ada@example.com>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trailer {
    pub token: String,
    pub value: String,
}

/// Everything the logic needs to know about a commit, independent of git.
///
/// `time_seconds` is the committer timestamp (seconds since the Unix epoch), because that is the
/// date `git log --since/--until` filters on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMeta {
    pub author: Author,
    pub time_seconds: i64,
    pub trailers: Vec<Trailer>,
}

/// Line and file changes for a single commit, equivalent to one commit's `git log --numstat`
/// output. Merge commits carry the default (all zero), matching git's default of emitting no
/// numstat for merges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffStat {
    pub insertions: u64,
    pub deletions: u64,
    pub files: u64,
}

/// Which column the stats table is sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SortBy {
    /// Sort by author name, A to Z
    Author,
    /// Sort by number of commits
    Commits,
    /// Sort by number of files touched
    Files,
    /// Sort by number of insertions
    Insertions,
    /// Sort by number of deletions
    Deletions,
    /// Sort by net lines of change
    Net,
}

/// Which version control system reads the repository and interprets the revision range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Vcs {
    /// Use jj in a jj workspace (colocated or not), git otherwise
    Auto,
    /// Read the git repository and take a git revision range
    Git,
    /// Read the jj workspace and take a jj revset
    Jj,
}

/// Parsed, validated options handed from the CLI down to the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Revision range, interpreted by the infrastructure: git revision syntax (e.g. `origin..HEAD`)
    /// or, in a jj workspace, a revset. `None` selects the backend's default.
    pub range: Option<String>,
    /// Group and display authors as `Name <email>` rather than just `Name`.
    pub email: bool,
    /// Also produce the reviewer/tester table from git trailers.
    pub reviews: bool,
    pub sort: SortBy,
    pub reverse: bool,
    /// Author regex patterns; a commit matches if any pattern matches. Empty means no author
    /// filtering.
    pub authors: Vec<String>,
    pub since: Option<String>,
    pub until: Option<String>,
}

/// Aggregated per-author statistics. Doubles as one row of the stats table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub author: String,
    pub commits: u64,
    pub num_files: u64,
    pub insertions: u64,
    pub deletions: u64,
    pub net: i64,
}

/// Aggregated per-reviewer statistics. Doubles as one row of the reviews table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub author: String,
    pub commits: u64,
}
