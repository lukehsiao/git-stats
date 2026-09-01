//! Value objects: plain data passed between the pure logic and the gix
//! infrastructure. Nothing here performs I/O or depends on gix.

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
/// `time_seconds` is the committer timestamp (seconds since the Unix epoch),
/// because that is the date `git log --since/--until` filters on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMeta {
    pub author: Author,
    pub time_seconds: i64,
    pub trailers: Vec<Trailer>,
}

/// Line and file changes for a single commit, equivalent to one commit's
/// `git log --numstat` output. Merge commits carry the default (all zero),
/// matching git's default of emitting no numstat for merges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffStat {
    pub insertions: u64,
    pub deletions: u64,
    pub files: u64,
}

/// Which column the stats table is sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SortBy {
    /// Sort by author alphabetic order
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

/// Parsed, validated options handed from the CLI down to the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Revision range, interpreted by the infrastructure (e.g. `origin..HEAD`).
    pub range: String,
    /// Group and display authors as `Name <email>` rather than just `Name`.
    pub email: bool,
    /// Also produce the reviewer/tester table from git trailers.
    pub reviews: bool,
    pub sort: SortBy,
    pub reverse: bool,
    /// Author regex patterns; a commit matches if any pattern matches. Empty
    /// means no author filtering.
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

/// Format a deletion count: `0` stays `0`, otherwise it is shown as `-n`.
#[must_use]
pub fn display_del(n: u64) -> String {
    match n {
        0 => "0".to_string(),
        n => format!("-{n}"),
    }
}

/// Format an insertion count: `0` stays `0`, otherwise it is shown as `+n`.
#[must_use]
pub fn display_add(n: u64) -> String {
    match n {
        0 => "0".to_string(),
        n => format!("+{n}"),
    }
}

/// Format a net line delta: positive values get a leading `+`, zero and
/// negative values render with their natural sign (`0`, `-5`).
#[must_use]
pub fn display_net(n: i64) -> String {
    if n > 0 {
        format!("+{n}")
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hegel::generators;

    #[hegel::test]
    fn display_add_signs_nonzero(tc: hegel::TestCase) {
        let n = tc.draw(generators::integers::<u64>());
        let rendered = display_add(n);
        if n == 0 {
            assert_eq!(rendered, "0");
        } else {
            assert_eq!(rendered, format!("+{n}"));
        }
    }

    #[hegel::test]
    fn display_del_signs_nonzero(tc: hegel::TestCase) {
        let n = tc.draw(generators::integers::<u64>());
        let rendered = display_del(n);
        if n == 0 {
            assert_eq!(rendered, "0");
        } else {
            assert_eq!(rendered, format!("-{n}"));
        }
    }

    /// Across the full `i64` range (including `MIN`, `MAX`, `0`), `display_net`
    /// never panics and follows its sign rules. This is precisely the case the
    /// old `_ => todo!()` arm claimed to need but could never reach.
    #[hegel::test]
    fn display_net_signs_and_never_panics(tc: hegel::TestCase) {
        let n = tc.draw(generators::integers::<i64>());
        let rendered = display_net(n);
        if n > 0 {
            assert_eq!(rendered, format!("+{n}"));
        } else {
            assert_eq!(rendered, n.to_string());
        }
    }
}
