use gix::date::Zoned;
use regex::Regex;

use crate::error::{Error, Result};
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
    /// Compile the author patterns and parse the date window. Each bound is optional and
    /// inclusive. Dates accept ISO 8601, RFC 2822, Unix timestamps, and relative spellings like "2
    /// weeks ago" (a documented subset of git's approxidate), which resolve against `now`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDate`] if a bound is not a recognized date, or
    /// [`Error::AuthorPattern`] if a pattern is not a valid regex.
    pub fn new(
        authors: &[String],
        since: Option<&str>,
        until: Option<&str>,
        now: &Zoned,
    ) -> Result<Self> {
        let since = since.map(|s| parse_date(s, now)).transpose()?;
        let until = until.map(|u| parse_date(u, now)).transpose()?;
        Ok(Self {
            authors: authors
                .iter()
                .map(|p| Regex::new(p).map_err(Error::from))
                .collect::<Result<_>>()?,
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

/// Parse a `--since`/`--until` date into seconds since the Unix epoch.
fn parse_date(input: &str, now: &Zoned) -> Result<i64> {
    gix::date::parse(input, Some(now.clone()))
        .map(|time| time.seconds)
        .map_err(|e| Error::InvalidDate {
            input: input.to_string(),
            message: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Contributor;
    use hegel::generators::{self, Generator};

    /// An arbitrary but fixed `now`. Its odd offset makes a test that depends on the time zone by
    /// accident stand out.
    fn now() -> Zoned {
        "2001-02-03T04:05:06+05:45[+05:45]".parse().unwrap()
    }

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

    fn committed_at(time_seconds: i64) -> CommitMeta {
        CommitMeta {
            author: Contributor {
                name: "Ada".to_string(),
                email: "ada@x".to_string(),
            },
            time_seconds,
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
        let since = since.map(|s| format!("@{s}"));
        let until = until.map(|u| format!("@{u}"));
        let filter = CommitFilter::new(&[], since.as_deref(), until.as_deref(), &now()).unwrap();
        assert_eq!(filter.keeps(&meta), want);
    }

    /// A relative date counts back from `now`, to the second.
    #[test]
    fn relative_dates_resolve_against_now() {
        let now = now();
        let day_ago = now.timestamp().as_second() - 24 * 60 * 60;
        let filter = CommitFilter::new(&[], Some("1 day ago"), None, &now).unwrap();
        assert!(filter.keeps(&committed_at(day_ago)));
        assert!(!filter.keeps(&committed_at(day_ago - 1)));
    }

    /// With several patterns, a commit is kept when its whole `Name <email>` identity matches any
    /// one of them.
    #[hegel::test]
    fn author_filter_matches_any_pattern_against_the_ident(tc: hegel::TestCase) {
        let meta = tc.draw(commit_meta().print_as_debug());
        let patterns = ["^a".to_string(), "ross@".to_string()];
        let filter = CommitFilter::new(&patterns, None, None, &now()).unwrap();
        let name = meta.author.name.as_str();
        let want = name.starts_with('a') || name == "bob ross";
        assert_eq!(filter.keeps(&meta), want);
    }

    #[test]
    fn an_invalid_pattern_is_an_error() {
        let err = CommitFilter::new(&["[".to_string()], None, None, &now()).unwrap_err();
        assert!(matches!(err, Error::AuthorPattern(_)), "{err:?}");
    }

    #[test]
    fn an_unparseable_date_is_an_error() {
        let err = CommitFilter::new(&[], None, Some("not-a-real-date"), &now()).unwrap_err();
        assert!(matches!(err, Error::InvalidDate { .. }), "{err:?}");
    }
}
