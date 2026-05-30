use regex::Regex;

use crate::model::CommitMeta;

/// Compile author regex patterns, surfacing a clear error on a bad pattern.
///
/// # Errors
///
/// Returns the underlying [`regex::Error`] if any pattern is not a valid regex.
pub fn compile_authors(patterns: &[String]) -> Result<Vec<Regex>, regex::Error> {
    patterns.iter().map(|p| Regex::new(p)).collect()
}

/// Indices of the commits to keep after author and date filtering.
///
/// A commit is kept when it matches at least one author pattern (or there are
/// none) and its committer time lies within `[since, until]` (each bound
/// optional). Mirrors `git log --author/--since/--until`.
pub fn keep_indices<'a>(
    metas: impl IntoIterator<Item = &'a CommitMeta>,
    authors: &[Regex],
    since: Option<i64>,
    until: Option<i64>,
) -> Vec<usize> {
    metas
        .into_iter()
        .enumerate()
        .filter(|(_, m)| {
            let author_ok = authors.is_empty() || {
                let ident = format!("{} <{}>", m.author.name, m.author.email);
                authors.iter().any(|re| re.is_match(&ident))
            };
            author_ok
                && since.is_none_or(|s| m.time_seconds >= s)
                && until.is_none_or(|u| m.time_seconds <= u)
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Author;
    use hegel::generators;

    #[hegel::composite]
    fn commit_metas(tc: hegel::TestCase) -> Vec<CommitMeta> {
        let n = tc.draw(generators::integers::<usize>().max_value(100));
        let mut metas = Vec::with_capacity(n);
        for _ in 0..n {
            // Multi-word names exercise filtering against idents that contain
            // spaces; some still begin with "a" so the `^a` test keeps a mix.
            let name = tc.draw(generators::sampled_from(vec![
                "ada lovelace",
                "amy pond",
                "bob ross",
                "carol kane",
            ]));
            metas.push(CommitMeta {
                author: Author {
                    name: name.to_string(),
                    email: format!("{name}@example.com"),
                },
                time_seconds: tc.draw(generators::integers::<i64>()),
                trailers: Vec::new(),
            });
        }
        metas
    }

    #[hegel::test]
    fn no_filters_keeps_everything(tc: hegel::TestCase) {
        let metas = tc.draw(commit_metas());
        let kept = keep_indices(metas.iter(), &[], None, None);
        assert_eq!(kept, (0..metas.len()).collect::<Vec<_>>());
    }

    #[hegel::test]
    fn kept_indices_are_sorted_and_in_range(tc: hegel::TestCase) {
        let metas = tc.draw(commit_metas());
        let since = tc.draw(generators::optional(generators::integers::<i64>()));
        let until = tc.draw(generators::optional(generators::integers::<i64>()));
        let kept = keep_indices(metas.iter(), &[], since, until);
        assert!(kept.iter().all(|&i| i < metas.len()));
        assert!(kept.windows(2).all(|w| w[0] < w[1]));
    }

    /// A commit is kept exactly when its committer time lies in `[since, until]`.
    #[hegel::test]
    fn date_window_membership(tc: hegel::TestCase) {
        let metas = tc.draw(commit_metas());
        let since = tc.draw(generators::optional(generators::integers::<i64>()));
        let until = tc.draw(generators::optional(generators::integers::<i64>()));
        let kept = keep_indices(metas.iter(), &[], since, until);
        for (i, m) in metas.iter().enumerate() {
            let want = since.is_none_or(|s| m.time_seconds >= s)
                && until.is_none_or(|u| m.time_seconds <= u);
            assert_eq!(kept.contains(&i), want);
        }
    }

    /// With one author pattern, kept commits are exactly those whose ident matches.
    #[hegel::test]
    fn author_filter_matches_ident(tc: hegel::TestCase) {
        let metas = tc.draw(commit_metas());
        let patterns = vec![Regex::new("^a").unwrap()];
        let kept = keep_indices(metas.iter(), &patterns, None, None);
        for (i, m) in metas.iter().enumerate() {
            let ident = format!("{} <{}>", m.author.name, m.author.email);
            assert_eq!(kept.contains(&i), patterns[0].is_match(&ident));
        }
    }
}
