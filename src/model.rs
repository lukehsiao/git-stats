//! Value objects: plain data passed between the pure logic and the gix infrastructure. Nothing here
//! performs I/O or depends on gix.

use clap::ValueEnum;

/// A person credited in a table: a commit's Author, resolved through `.mailmap` to match git's
/// `%aN`/`%aE`, or a Reviewer named in a trailer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Contributor {
    pub name: String,
    pub email: String,
}

impl Contributor {
    /// The full identity, `Name <email>`, that `--author` patterns match against.
    #[must_use]
    pub fn ident(&self) -> String {
        format!("{} <{}>", self.name, self.email)
    }

    /// The Contributor key that groups and labels a table row: `Name <email>` with `email`, else
    /// `Name`. A Contributor without an email is keyed by the name alone in both modes.
    ///
    /// An empty name uses the email instead, so the row does not show as blank. This differs from
    /// `git shortlog`, which keys such a Contributor as `""`, or ` <email>` with `email`. It also
    /// splits rows: without `email`, git groups every empty-name Contributor into one row, and here
    /// each email gets its own. A missing email differs too: `git shortlog -e` shows `Name <>`.
    #[must_use]
    pub fn key(&self, email: bool) -> String {
        if self.name.is_empty() {
            self.email.clone()
        } else if email && !self.email.is_empty() {
            self.ident()
        } else {
            self.name.clone()
        }
    }
}

/// Everything the logic needs to know about a commit, independent of git.
///
/// `time_seconds` is the committer timestamp (seconds since the Unix epoch), because that is the
/// date `git log --since/--until` filters on. `reviewers` holds one entry per `Acked-by`,
/// `Tested-by`, or `Reviewed-by` trailer, in message order, so a Reviewer named twice appears
/// twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMeta {
    pub author: Contributor,
    pub time_seconds: i64,
    pub reviewers: Vec<Contributor>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn who(name: &str, email: &str) -> Contributor {
        Contributor {
            name: name.to_string(),
            email: email.to_string(),
        }
    }

    #[test]
    fn key_shows_the_email_only_when_asked() {
        let ada = who("Ada Lovelace", "ada@x");
        assert_eq!(ada.key(false), "Ada Lovelace");
        assert_eq!(ada.key(true), "Ada Lovelace <ada@x>");
    }

    #[test]
    fn an_empty_name_is_keyed_by_the_email() {
        let anon = who("", "anon@x");
        assert_eq!(anon.key(false), "anon@x");
        assert_eq!(anon.key(true), "anon@x");
    }

    #[test]
    fn a_missing_email_is_left_out_of_the_key() {
        let ada = who("Ada", "");
        assert_eq!(ada.key(false), "Ada");
        assert_eq!(ada.key(true), "Ada");
    }

    /// `--author` matches the whole identity, whatever the key shows.
    #[test]
    fn ident_always_has_both_parts() {
        assert_eq!(who("Ada", "ada@x").ident(), "Ada <ada@x>");
        assert_eq!(who("", "anon@x").ident(), " <anon@x>");
    }
}
