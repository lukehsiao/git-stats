use regex::Regex;

use crate::model::CommitMeta;

/// Which commits count toward the stats table: those by a matching author, committed inside a date
/// window. Mirrors `git log --author/--since/--until`.
#[derive(Debug, Clone)]
pub struct CommitFilter {
    authors: Vec<Regex>,
    since: Option<i64>,
    until: Option<i64>,
}

impl CommitFilter {
    /// Compile the author patterns and take the date window, in seconds since the Unix epoch. Each
    /// bound is optional and inclusive.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`regex::Error`] if any pattern is not a valid regex.
    pub fn new(
        authors: &[String],
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<Self, regex::Error> {
        Ok(Self {
            authors: authors
                .iter()
                .map(|p| Regex::new(p))
                .collect::<Result<_, _>>()?,
            since,
            until,
        })
    }

    /// Whether `meta` counts: its author's identity matches at least one pattern (or there are
    /// none), and its committer time lies within the window.
    #[must_use]
    pub fn keeps(&self, meta: &CommitMeta) -> bool {
        let author_ok = self.authors.is_empty() || {
            let ident = meta.author.ident();
            self.authors.iter().any(|re| re.is_match(&ident))
        };
        author_ok
            && self.since.is_none_or(|s| meta.time_seconds >= s)
            && self.until.is_none_or(|u| meta.time_seconds <= u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Contributor;
    use hegel::generators::{self, Generator};

    /// Multi-word names exercise filtering against idents that contain spaces. Some begin with "a",
    /// so the `^a` pattern keeps a mix.
    #[hegel::composite]
    fn commit_meta(tc: &hegel::TestCase) -> CommitMeta {
        let name = tc.draw(generators::sampled_from(vec![
            "ada lovelace",
            "amy pond",
            "bob ross",
            "carol kane",
        ]));
        CommitMeta {
            author: Contributor {
                name: name.to_string(),
                email: format!("{name}@example.com"),
            },
            time_seconds: tc.draw(generators::integers::<i64>()),
            reviewers: Vec::new(),
        }
    }

    /// A commit is kept exactly when its committer time lies in `[since, until]`.
    #[hegel::test]
    fn date_window_membership(tc: hegel::TestCase) {
        let meta = tc.draw(commit_meta().print_as_debug());
        let since = tc.draw(generators::optional(generators::integers::<i64>()));
        let until = tc.draw(generators::optional(generators::integers::<i64>()));
        let want = since.is_none_or(|s| meta.time_seconds >= s)
            && until.is_none_or(|u| meta.time_seconds <= u);
        let filter = CommitFilter::new(&[], since, until).unwrap();
        assert_eq!(filter.keeps(&meta), want);
    }

    /// With several patterns, a commit is kept when its whole `Name <email>` identity matches any
    /// one of them.
    #[hegel::test]
    fn author_filter_matches_any_pattern_against_the_ident(tc: hegel::TestCase) {
        let meta = tc.draw(commit_meta().print_as_debug());
        let patterns = ["^a".to_string(), "ross@".to_string()];
        let filter = CommitFilter::new(&patterns, None, None).unwrap();
        let name = meta.author.name.as_str();
        let want = name.starts_with('a') || name == "bob ross";
        assert_eq!(filter.keeps(&meta), want);
    }

    #[test]
    fn an_invalid_pattern_is_an_error() {
        assert!(CommitFilter::new(&["[".to_string()], None, None).is_err());
    }
}
