use std::collections::HashMap;

use crate::model::{CommitMeta, DiffStat, Review, SortBy, Stat};

/// One commit's contribution to the stats table: its Author's Contributor key and its numstat.
#[derive(Debug, Clone)]
pub struct CommitStat {
    pub key: String,
    pub diff: DiffStat,
}

/// The stats table: one row per Author in display order, and the Total row kept apart so that no
/// Author row can be mistaken for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsTable {
    pub rows: Vec<Stat>,
    pub total: Stat,
}

/// Everything a run shows: the stats table, absent when no commit survives the filters, and the
/// reviews table, empty when not requested or when no commit has a review trailer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub stats: Option<StatsTable>,
    pub reviews: Vec<Review>,
}

/// Build the stats table from per-commit stats: one row per Author, sorted by `sort`, plus the
/// Total row. Returns `None` when there are no commits, since an empty table is not shown at all.
///
/// Counts sort descending and names A to Z, by byte value. With `reverse`, the column order is
/// flipped. Ties sort by name, A to Z in both directions, so the order does not depend on which
/// commit the walk saw first. Sums saturate rather than overflow.
#[must_use]
pub fn stats_table(commits: &[CommitStat], sort: SortBy, reverse: bool) -> Option<StatsTable> {
    if commits.is_empty() {
        return None;
    }
    let mut by_key: HashMap<&str, Stat> = HashMap::new();
    let mut total = empty_stat("Total");
    for c in commits {
        add(
            by_key.entry(&c.key).or_insert_with_key(|k| empty_stat(k)),
            c.diff,
        );
        add(&mut total, c.diff);
    }
    let mut rows: Vec<Stat> = by_key.into_values().collect();
    for s in rows.iter_mut().chain([&mut total]) {
        s.net = net(s.insertions, s.deletions);
    }
    // Every row has a distinct name, so the name tie-break makes the order total.
    rows.sort_unstable_by(|a, b| {
        let column = match sort {
            SortBy::Author => a.author.cmp(&b.author),
            SortBy::Commits => b.commits.cmp(&a.commits),
            SortBy::Files => b.num_files.cmp(&a.num_files),
            SortBy::Insertions => b.insertions.cmp(&a.insertions),
            SortBy::Deletions => b.deletions.cmp(&a.deletions),
            SortBy::Net => b.net.cmp(&a.net),
        };
        let column = if reverse { column.reverse() } else { column };
        column.then_with(|| a.author.cmp(&b.author))
    });
    Some(StatsTable { rows, total })
}

/// Build the reviews table: per Reviewer, the commits they signed off on. A Reviewer is credited at
/// most once per commit even if several of its trailers name them. Rows come in descending commit
/// count, ties by name, A to Z.
#[must_use]
pub fn reviews_table<'a>(
    metas: impl IntoIterator<Item = &'a CommitMeta>,
    email: bool,
) -> Vec<Review> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut credited: Vec<String> = Vec::new();
    for m in metas {
        for r in &m.reviewers {
            let key = r.key(email);
            if !credited.contains(&key) {
                credited.push(key);
            }
        }
        for key in credited.drain(..) {
            let n = counts.entry(key).or_default();
            *n = n.saturating_add(1);
        }
    }
    let mut rows: Vec<Review> = counts
        .into_iter()
        .map(|(author, commits)| Review { author, commits })
        .collect();
    // Every row has a distinct name, so the name tie-break makes the order total.
    rows.sort_unstable_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then_with(|| a.author.cmp(&b.author))
    });
    rows
}

fn empty_stat(author: &str) -> Stat {
    Stat {
        author: author.to_string(),
        commits: 0,
        num_files: 0,
        insertions: 0,
        deletions: 0,
        net: 0,
    }
}

fn add(s: &mut Stat, diff: DiffStat) {
    s.commits = s.commits.saturating_add(1);
    s.num_files = s.num_files.saturating_add(diff.files);
    s.insertions = s.insertions.saturating_add(diff.insertions);
    s.deletions = s.deletions.saturating_add(diff.deletions);
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
    use crate::model::Contributor;
    use hegel::generators::{self, Generator};
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, BTreeSet};

    /// A small author pool forces grouping. It has multi-word names, so grouping and ordering see
    /// spaces, and names that differ only in case, so ordering sees byte order. `u32`-range diffs
    /// keep the sums of up to 200 commits inside `u64` and `i64`, so the test's own arithmetic
    /// cannot overflow before the code under test runs.
    #[hegel::composite]
    fn commit_list(tc: &hegel::TestCase) -> Vec<CommitStat> {
        const NAMES: [&str; 6] = [
            "Ada Lovelace",
            "Grace Hopper",
            "Bob",
            "Carol Shaw",
            "Don",
            "ada",
        ];
        let n = tc.draw(generators::integers::<usize>().max_value(200));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            let who = tc.draw(generators::integers::<usize>().max_value(NAMES.len() - 1));
            commits.push(CommitStat {
                key: NAMES[who].to_string(),
                diff: DiffStat {
                    insertions: u64::from(tc.draw(generators::integers::<u32>())),
                    deletions: u64::from(tc.draw(generators::integers::<u32>())),
                    files: u64::from(tc.draw(generators::integers::<u32>())),
                },
            });
        }
        commits
    }

    fn any_sort_by() -> impl Generator<SortBy> {
        generators::sampled_from(vec![
            SortBy::Author,
            SortBy::Commits,
            SortBy::Files,
            SortBy::Insertions,
            SortBy::Deletions,
            SortBy::Net,
        ])
    }

    fn signed(n: u64) -> i64 {
        i64::try_from(n).unwrap()
    }

    #[test]
    fn no_commits_build_no_table() {
        assert_eq!(stats_table(&[], SortBy::Commits, false), None);
    }

    /// Each row holds exactly its Author's sums, whatever the sort.
    #[hegel::test]
    fn rows_match_independent_per_author_sums(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list().print_as_debug());
        let by = tc.draw(any_sort_by().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        let mut expected: BTreeMap<&str, (u64, u64, u64, u64)> = BTreeMap::new();
        for c in &commits {
            let e = expected.entry(c.key.as_str()).or_default();
            e.0 += 1;
            e.1 += c.diff.files;
            e.2 += c.diff.insertions;
            e.3 += c.diff.deletions;
        }
        let rows = stats_table(&commits, by, reverse)
            .map(|t| t.rows)
            .unwrap_or_default();
        let actual: BTreeMap<&str, (u64, u64, u64, u64)> = rows
            .iter()
            .map(|s| {
                assert_eq!(s.net, signed(s.insertions) - signed(s.deletions), "{s:?}");
                (
                    s.author.as_str(),
                    (s.commits, s.num_files, s.insertions, s.deletions),
                )
            })
            .collect();
        assert_eq!(rows.len(), actual.len(), "duplicate rows: {rows:?}");
        assert_eq!(actual, expected);
    }

    #[hegel::test]
    fn total_matches_independent_sums(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list().print_as_debug());
        let Some(table) = stats_table(&commits, SortBy::Commits, false) else {
            assert!(commits.is_empty());
            return;
        };
        let ins: u64 = commits.iter().map(|c| c.diff.insertions).sum();
        let del: u64 = commits.iter().map(|c| c.diff.deletions).sum();
        let total = table.total;
        assert_eq!(total.author, "Total");
        assert_eq!(total.commits, u64::try_from(commits.len()).unwrap());
        assert_eq!(total.num_files, commits.iter().map(|c| c.diff.files).sum());
        assert_eq!(total.insertions, ins);
        assert_eq!(total.deletions, del);
        assert_eq!(total.net, signed(ins) - signed(del));
    }

    /// How `a` compares to `b` in the column `by` names, ascending.
    fn column(a: &Stat, b: &Stat, by: SortBy) -> Ordering {
        match by {
            SortBy::Author => a.author.cmp(&b.author),
            SortBy::Commits => a.commits.cmp(&b.commits),
            SortBy::Files => a.num_files.cmp(&b.num_files),
            SortBy::Insertions => a.insertions.cmp(&b.insertions),
            SortBy::Deletions => a.deletions.cmp(&b.deletions),
            SortBy::Net => a.net.cmp(&b.net),
        }
    }

    /// By default, counts are non-increasing and names non-decreasing (A to Z). `reverse` flips
    /// both. Rows that tie in the column are in name order, A to Z, in both directions. The order
    /// is strict, so it cannot depend on the order in which git or jj walks the commits.
    #[hegel::test]
    fn rows_follow_the_column_then_the_name(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list().print_as_debug());
        let by = tc.draw(any_sort_by().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        let rows = stats_table(&commits, by, reverse)
            .map(|t| t.rows)
            .unwrap_or_default();
        let ascending = (by == SortBy::Author) != reverse;
        for w in rows.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ord = column(a, b, by);
            let ord = if ascending { ord } else { ord.reverse() };
            assert!(
                ord.is_lt() || (ord.is_eq() && a.author < b.author),
                "{by:?} reverse={reverse}: {a:?} then {b:?}"
            );
        }
    }

    /// The saturating sums and the clamped `net` exist so that counts near `u64::MAX` cannot panic,
    /// so the generator draws the full `u64` range.
    #[hegel::test]
    fn stats_table_never_panics_on_boundary_values(tc: hegel::TestCase) {
        let n = tc.draw(generators::integers::<usize>().max_value(20));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            commits.push(CommitStat {
                key: "boundary".to_string(),
                diff: DiffStat {
                    insertions: tc.draw(generators::integers::<u64>()),
                    deletions: tc.draw(generators::integers::<u64>()),
                    files: tc.draw(generators::integers::<u64>()),
                },
            });
        }
        let _ = stats_table(&commits, SortBy::Net, false);
    }

    /// Commits whose Reviewers come from a small pool, so ties in commit count are common, and a
    /// commit can name the same Reviewer twice. The pool has names that differ only in case.
    #[hegel::composite]
    fn reviewed_commits(tc: &hegel::TestCase) -> Vec<CommitMeta> {
        const REVIEWERS: [(&str, &str); 4] = [
            ("Zed", "z@x"),
            ("Ada", "a@x"),
            ("Bob", "b@x"),
            ("ada", "l@x"),
        ];
        let n = tc.draw(generators::integers::<usize>().max_value(20));
        let mut metas = Vec::with_capacity(n);
        for _ in 0..n {
            let picks = tc.draw(
                generators::vecs(generators::integers::<usize>().max_value(REVIEWERS.len() - 1))
                    .max_size(3),
            );
            metas.push(CommitMeta {
                author: Contributor {
                    name: "Author".to_string(),
                    email: "author@x".to_string(),
                },
                time_seconds: 0,
                reviewers: picks
                    .into_iter()
                    .map(|i| Contributor {
                        name: REVIEWERS[i].0.to_string(),
                        email: REVIEWERS[i].1.to_string(),
                    })
                    .collect(),
            });
        }
        metas
    }

    /// Each Reviewer's count is the number of commits that name them at least once.
    #[hegel::test]
    fn reviews_count_each_commit_once(tc: hegel::TestCase) {
        let metas = tc.draw(reviewed_commits().print_as_debug());
        let email = tc.draw(generators::booleans());
        let mut expected: BTreeMap<String, u64> = BTreeMap::new();
        for m in &metas {
            let keys: BTreeSet<String> = m.reviewers.iter().map(|r| r.key(email)).collect();
            for key in keys {
                *expected.entry(key).or_default() += 1;
            }
        }
        let actual: BTreeMap<String, u64> = reviews_table(&metas, email)
            .into_iter()
            .map(|r| (r.author, r.commits))
            .collect();
        assert_eq!(actual, expected);
    }

    /// Reviewers come in descending commit count, and ties come A to Z. The order is strict, so it
    /// cannot depend on the order in which git or jj walks the commits.
    #[hegel::test]
    fn reviews_sort_by_count_then_name(tc: hegel::TestCase) {
        let metas = tc.draw(reviewed_commits().print_as_debug());
        let email = tc.draw(generators::booleans());
        for w in reviews_table(&metas, email).windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!(
                a.commits > b.commits || (a.commits == b.commits && a.author < b.author),
                "{a:?} then {b:?}"
            );
        }
    }
}
