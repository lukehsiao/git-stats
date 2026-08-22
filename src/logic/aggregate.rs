use std::collections::HashMap;

use crate::model::{Author, CommitMeta, DiffStat, Review, Stat};

/// One commit's contribution to the aggregate: its grouping key and numstat.
#[derive(Debug, Clone)]
pub struct CommitStat {
    pub author_key: String,
    pub diff: DiffStat,
}

/// Trailer tokens (lowercased) that count as a review/test sign-off.
const REVIEW_TOKENS: [&str; 3] = ["acked-by", "tested-by", "reviewed-by"];

/// The author display and grouping key: `Name <email>` when `email`, else `Name`.
#[must_use]
pub fn author_key(author: &Author, email: bool) -> String {
    if email {
        format!("{} <{}>", author.name, author.email)
    } else {
        author.name.clone()
    }
}

/// Aggregate per-commit stats into per-author totals, returned in first-seen
/// order (callers sort afterwards). Sums saturate rather than overflow.
#[must_use]
pub fn aggregate(commits: &[CommitStat]) -> Vec<Stat> {
    let mut stats: Vec<Stat> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for c in commits {
        let i = *index.entry(c.author_key.as_str()).or_insert_with(|| {
            stats.push(Stat {
                author: c.author_key.clone(),
                commits: 0,
                num_files: 0,
                insertions: 0,
                deletions: 0,
                net: 0,
            });
            stats.len() - 1
        });
        let s = &mut stats[i];
        s.commits = s.commits.saturating_add(1);
        s.num_files = s.num_files.saturating_add(c.diff.files);
        s.insertions = s.insertions.saturating_add(c.diff.insertions);
        s.deletions = s.deletions.saturating_add(c.diff.deletions);
    }
    for s in &mut stats {
        s.net = net(s.insertions, s.deletions);
    }
    stats
}

/// Sum per-author rows into a single "Total" row.
#[must_use]
pub fn compute_totals(stats: &[Stat]) -> Stat {
    let mut total = Stat {
        author: "Total".to_string(),
        commits: 0,
        num_files: 0,
        insertions: 0,
        deletions: 0,
        net: 0,
    };
    for s in stats {
        total.commits = total.commits.saturating_add(s.commits);
        total.num_files = total.num_files.saturating_add(s.num_files);
        total.insertions = total.insertions.saturating_add(s.insertions);
        total.deletions = total.deletions.saturating_add(s.deletions);
    }
    total.net = net(total.insertions, total.deletions);
    total
}

/// Count, per reviewer, the commits they signed off on via Acked-by / Tested-by
/// / Reviewed-by trailers. A reviewer is credited at most once per commit even
/// if they appear in several of those trailers. Returned in descending commit
/// count, ties broken by first-seen order.
#[must_use]
pub fn aggregate_reviews<'a>(
    metas: impl IntoIterator<Item = &'a CommitMeta>,
    email: bool,
) -> Vec<Review> {
    let mut reviews: Vec<Review> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for m in metas {
        let mut credited: Vec<String> = Vec::new();
        for t in &m.trailers {
            if !REVIEW_TOKENS
                .iter()
                .any(|token| t.token.eq_ignore_ascii_case(token))
            {
                continue;
            }
            let key = reviewer_key(&t.value, email);
            if credited.contains(&key) {
                continue;
            }
            credited.push(key.clone());
            let i = *index.entry(key.clone()).or_insert_with(|| {
                reviews.push(Review {
                    author: key.clone(),
                    commits: 0,
                });
                reviews.len() - 1
            });
            reviews[i].commits = reviews[i].commits.saturating_add(1);
        }
    }
    reviews.sort_by_key(|r| std::cmp::Reverse(r.commits));
    reviews
}

/// The reviewer key from a trailer value: the full `Name <email>` when `email`,
/// otherwise just the name preceding ` <`.
fn reviewer_key(value: &str, email: bool) -> String {
    let value = value.trim();
    if email {
        return value.to_string();
    }
    match value.split_once(" <") {
        Some((name, _)) => name.trim().to_string(),
        None => value.to_string(),
    }
}

/// Net line delta, clamped so absurd inputs cannot overflow (panic-free).
fn net(insertions: u64, deletions: u64) -> i64 {
    let ins = i64::try_from(insertions).unwrap_or(i64::MAX);
    let del = i64::try_from(deletions).unwrap_or(i64::MAX);
    ins - del
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DiffStat;
    use hegel::generators;
    use std::collections::{BTreeMap, BTreeSet};

    /// A small author pool forces grouping; the names include multi-word
    /// entries so grouping is exercised with spaces. `u32`-range diffs keep the
    /// sums of up to 200 commits comfortably inside `u64` and `i64`, so the
    /// test's own arithmetic cannot overflow before the code under test runs.
    #[hegel::composite]
    fn commit_list(tc: &hegel::TestCase) -> Vec<CommitStat> {
        const NAMES: [&str; 5] = ["Ada Lovelace", "Grace Hopper", "Bob", "Carol Shaw", "Don"];
        let n = tc.draw(generators::integers::<usize>().max_value(200));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            let who = tc.draw(generators::integers::<usize>().max_value(NAMES.len() - 1));
            commits.push(CommitStat {
                author_key: NAMES[who].to_string(),
                diff: DiffStat {
                    insertions: u64::from(tc.draw(generators::integers::<u32>())),
                    deletions: u64::from(tc.draw(generators::integers::<u32>())),
                    files: u64::from(tc.draw(generators::integers::<u32>())),
                },
            });
        }
        commits
    }

    fn fingerprint(stats: &[Stat]) -> BTreeMap<String, (u64, u64, u64, u64, i64)> {
        stats
            .iter()
            .map(|s| {
                (
                    s.author.clone(),
                    (s.commits, s.num_files, s.insertions, s.deletions, s.net),
                )
            })
            .collect()
    }

    #[hegel::test]
    fn totals_match_independent_sums(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list());
        let totals = compute_totals(&aggregate(&commits));

        let exp_ins: u64 = commits.iter().map(|c| c.diff.insertions).sum();
        let exp_del: u64 = commits.iter().map(|c| c.diff.deletions).sum();

        assert_eq!(totals.commits, u64::try_from(commits.len()).unwrap());
        assert_eq!(totals.insertions, exp_ins);
        assert_eq!(totals.deletions, exp_del);
        assert_eq!(totals.num_files, commits.iter().map(|c| c.diff.files).sum());
        assert_eq!(
            totals.net,
            i64::try_from(exp_ins).unwrap() - i64::try_from(exp_del).unwrap()
        );
    }

    #[hegel::test]
    fn per_stat_net_is_insertions_minus_deletions(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list());
        for s in aggregate(&commits) {
            assert_eq!(
                s.net,
                i64::try_from(s.insertions).unwrap() - i64::try_from(s.deletions).unwrap()
            );
        }
    }

    #[hegel::test]
    fn one_row_per_distinct_author(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list());
        let distinct: BTreeSet<&str> = commits.iter().map(|c| c.author_key.as_str()).collect();
        assert_eq!(aggregate(&commits).len(), distinct.len());
    }

    #[hegel::test]
    fn aggregation_is_order_independent(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list());
        let forward = aggregate(&commits);
        let mut reversed = commits;
        reversed.reverse();
        let backward = aggregate(&reversed);
        assert_eq!(fingerprint(&forward), fingerprint(&backward));
    }

    /// `reviewer_key` keeps a full multi-word name (spaces and all). With an
    /// email present and `email=false` it strips the ` <email>`; with `email=true`
    /// it keeps the whole value. When the trailer carries only a bare name (no
    /// ` <email>`) both modes return the name unchanged. The name generator
    /// produces 1-4 words so the ` <` split is exercised against names that
    /// themselves contain spaces, and `with_email` exercises both branches.
    #[hegel::test]
    fn reviewer_key_handles_multiword_names(tc: hegel::TestCase) {
        let name = tc.draw(generators::from_regex(r"[A-Za-z]+( [A-Za-z]+){0,3}").fullmatch(true));
        if tc.draw(generators::booleans()) {
            let email = tc.draw(generators::from_regex(r"[a-z]+@[a-z]+\.[a-z]+").fullmatch(true));
            let value = format!("{name} <{email}>");
            assert_eq!(reviewer_key(&value, false), name);
            assert_eq!(reviewer_key(&value, true), value);
        } else {
            // No email in the trailer: both modes return the name unchanged.
            assert_eq!(reviewer_key(&name, false), name);
            assert_eq!(reviewer_key(&name, true), name);
        }
    }

    /// Aggregation must stay panic-free even when counts approach `u64::MAX`.
    /// The saturating sums and the clamped `net` exist precisely for this, so
    /// the generator draws the full `u64` range (including the boundaries).
    #[hegel::test]
    fn aggregate_never_panics_on_boundary_values(tc: hegel::TestCase) {
        let n = tc.draw(generators::integers::<usize>().max_value(20));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            commits.push(CommitStat {
                author_key: "boundary".to_string(),
                diff: DiffStat {
                    insertions: tc.draw(generators::integers::<u64>()),
                    deletions: tc.draw(generators::integers::<u64>()),
                    files: tc.draw(generators::integers::<u64>()),
                },
            });
        }
        // Neither call may panic; net is clamped, sums saturate.
        let _ = compute_totals(&aggregate(&commits));
    }
}
