use std::collections::HashMap;

use clap::ValueEnum;

use crate::model::{CommitMeta, Contributor, DiffStat};

/// A column of the stats table, in display order, and what `--sort` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Column {
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

impl Column {
    /// Every column, in display order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        Self::value_variants()
    }
}

/// The sums in one row of the stats table: an Author's, or the Total row's. Sums saturate rather
/// than overflow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub commits: u64,
    pub files: u64,
    pub insertions: u64,
    pub deletions: u64,
}

impl Counts {
    /// The net line delta, insertions minus deletions. Each side is clamped to `i64::MAX` first,
    /// so the difference cannot overflow.
    #[must_use]
    pub fn net(&self) -> i64 {
        let ins = i64::try_from(self.insertions).unwrap_or(i64::MAX);
        let del = i64::try_from(self.deletions).unwrap_or(i64::MAX);
        ins - del
    }

    fn add(&mut self, diff: DiffStat) {
        self.commits = self.commits.saturating_add(1);
        self.files = self.files.saturating_add(diff.files);
        self.insertions = self.insertions.saturating_add(diff.insertions);
        self.deletions = self.deletions.saturating_add(diff.deletions);
    }
}

/// One Author's row of the stats table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorRow {
    /// The Author's Contributor key.
    pub key: String,
    pub counts: Counts,
}

/// One row of the reviews table: a Reviewer's Contributor key and the commits they reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub key: String,
    pub commits: u64,
}

/// The stats table: one row per Author, in display order, and the Total row, which sums them. Only
/// [`stats_table`] builds one, so those invariants always hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsTable {
    rows: Vec<AuthorRow>,
    total: Counts,
}

impl StatsTable {
    /// The Author rows, in display order. Each has a distinct Contributor key.
    #[must_use]
    pub fn rows(&self) -> &[AuthorRow] {
        &self.rows
    }

    /// The Total row: the sums over every Author row.
    #[must_use]
    pub fn total(&self) -> Counts {
        self.total
    }
}

/// Everything a run shows: the stats table, absent when no commit survives the filters, the
/// reviews table, empty when not requested or when no commit has a review trailer, and whether
/// the range reached the shallow boundary, which calls for a warning that history is missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub stats: Option<StatsTable>,
    pub reviews: Vec<Review>,
    pub reached_shallow_boundary: bool,
}

/// Build the stats table from each commit's metadata and numstat: one row per Author, keyed by
/// [`Contributor::key`] with `email`, sorted by `sort`, plus the Total row. Returns `None` when
/// there are no commits, since an empty table is not shown at all.
///
/// Counts sort descending and keys A to Z, by byte value. With `reverse`, the column order is
/// flipped. Ties sort by key, A to Z in both directions, so the order does not depend on which
/// commit the walk saw first.
#[must_use]
pub fn stats_table(
    commits: &[(CommitMeta, DiffStat)],
    email: bool,
    sort: Column,
    reverse: bool,
) -> Option<StatsTable> {
    if commits.is_empty() {
        return None;
    }
    let mut by_key: HashMap<String, Counts> = HashMap::new();
    let mut total = Counts::default();
    for (meta, diff) in commits {
        by_key.entry(meta.author.key(email)).or_default().add(*diff);
        total.add(*diff);
    }
    let mut rows: Vec<AuthorRow> = by_key
        .into_iter()
        .map(|(key, counts)| AuthorRow { key, counts })
        .collect();
    // Every row has a distinct key, so the key tie-break makes the order total.
    rows.sort_unstable_by(|a, b| {
        let (x, y) = (&a.counts, &b.counts);
        let column = match sort {
            Column::Author => a.key.cmp(&b.key),
            Column::Commits => y.commits.cmp(&x.commits),
            Column::Files => y.files.cmp(&x.files),
            Column::Insertions => y.insertions.cmp(&x.insertions),
            Column::Deletions => y.deletions.cmp(&x.deletions),
            Column::Net => y.net().cmp(&x.net()),
        };
        let column = if reverse { column.reverse() } else { column };
        column.then_with(|| a.key.cmp(&b.key))
    });
    Some(StatsTable { rows, total })
}

/// Build the reviews table from each commit's Reviewers: per Reviewer, keyed by
/// [`Contributor::key`] with `email`, the commits they signed off on. A Reviewer is credited at most
/// once per commit even if several of its trailers name them. Rows come in descending commit count,
/// ties by key, A to Z.
#[must_use]
pub fn reviews_table(reviewers: &[Vec<Contributor>], email: bool) -> Vec<Review> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut credited: Vec<String> = Vec::new();
    for named in reviewers {
        for r in named {
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
        .map(|(key, commits)| Review { key, commits })
        .collect();
    // Every row has a distinct key, so the key tie-break makes the order total.
    rows.sort_unstable_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.key.cmp(&b.key)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use hegel::generators::{self, Generator};
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, BTreeSet};

    fn who(name: &str, email: &str) -> Contributor {
        Contributor {
            name: name.to_string(),
            email: email.to_string(),
        }
    }

    fn authored_by(author: Contributor) -> CommitMeta {
        CommitMeta {
            author,
            time_seconds: 0,
        }
    }

    /// A small Author pool forces grouping. It has multi-word names, so grouping and ordering see
    /// spaces, and names that differ only in case, so ordering sees byte order. One name has two
    /// emails, so `email` decides whether its commits share a row, and one name is empty, so its
    /// row is keyed by the email. `u32`-range diffs keep the sums of up to 200 commits inside `u64`
    /// and `i64`, so the test's own arithmetic cannot overflow before the code under test runs.
    #[hegel::composite]
    fn commit_list(tc: &hegel::TestCase) -> Vec<(CommitMeta, DiffStat)> {
        const AUTHORS: [(&str, &str); 8] = [
            ("Ada Lovelace", "ada@x"),
            ("Ada Lovelace", "lovelace@y"),
            ("Grace Hopper", "grace@x"),
            ("Bob", "bob@x"),
            ("Carol Shaw", "carol@x"),
            ("Don", "don@x"),
            ("ada", "lower@x"),
            ("", "anon@x"),
        ];
        let n = tc.draw(generators::integers::<usize>().max_value(200));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            let (name, email) =
                AUTHORS[tc.draw(generators::integers::<usize>().max_value(AUTHORS.len() - 1))];
            let diff = DiffStat {
                insertions: u64::from(tc.draw(generators::integers::<u32>())),
                deletions: u64::from(tc.draw(generators::integers::<u32>())),
                files: u64::from(tc.draw(generators::integers::<u32>())),
            };
            commits.push((authored_by(who(name, email)), diff));
        }
        commits
    }

    fn any_column() -> impl Generator<Column> {
        generators::sampled_from(Column::all().to_vec())
    }

    fn signed(n: u64) -> i64 {
        i64::try_from(n).unwrap()
    }

    #[test]
    fn no_commits_build_no_table() {
        assert_eq!(stats_table(&[], false, Column::Commits, false), None);
    }

    /// Each row holds exactly the sums of the commits whose Author has its Contributor key, whatever
    /// the sort.
    #[hegel::test]
    fn rows_match_independent_per_author_sums(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list().print_as_debug());
        let email = tc.draw(generators::booleans());
        let by = tc.draw(any_column().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        let mut expected: BTreeMap<String, Counts> = BTreeMap::new();
        for (meta, diff) in &commits {
            let e = expected.entry(meta.author.key(email)).or_default();
            e.commits += 1;
            e.files += diff.files;
            e.insertions += diff.insertions;
            e.deletions += diff.deletions;
        }
        let table = stats_table(&commits, email, by, reverse);
        let rows = table.as_ref().map_or(&[][..], StatsTable::rows);
        let actual: BTreeMap<String, Counts> =
            rows.iter().map(|r| (r.key.clone(), r.counts)).collect();
        assert_eq!(rows.len(), actual.len(), "duplicate rows: {rows:?}");
        assert_eq!(actual, expected);
    }

    #[hegel::test]
    fn total_matches_independent_sums(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list().print_as_debug());
        let Some(table) = stats_table(&commits, false, Column::Commits, false) else {
            assert_eq!(commits, []);
            return;
        };
        let ins: u64 = commits.iter().map(|(_, d)| d.insertions).sum();
        let del: u64 = commits.iter().map(|(_, d)| d.deletions).sum();
        let total = table.total();
        let expected = Counts {
            commits: u64::try_from(commits.len()).unwrap(),
            files: commits.iter().map(|(_, d)| d.files).sum(),
            insertions: ins,
            deletions: del,
        };
        assert_eq!(total, expected);
        assert_eq!(total.net(), signed(ins) - signed(del));
    }

    /// How `a` compares to `b` in the column `by`, ascending.
    fn column(a: &AuthorRow, b: &AuthorRow, by: Column) -> Ordering {
        let (x, y) = (&a.counts, &b.counts);
        match by {
            Column::Author => a.key.cmp(&b.key),
            Column::Commits => x.commits.cmp(&y.commits),
            Column::Files => x.files.cmp(&y.files),
            Column::Insertions => x.insertions.cmp(&y.insertions),
            Column::Deletions => x.deletions.cmp(&y.deletions),
            Column::Net => (signed(x.insertions) - signed(x.deletions))
                .cmp(&(signed(y.insertions) - signed(y.deletions))),
        }
    }

    /// By default, counts are non-increasing and names non-decreasing (A to Z). `reverse` flips
    /// both. Rows that tie in the column are in name order, A to Z, in both directions. The order
    /// is strict, so it cannot depend on the order in which git or jj walks the commits.
    #[hegel::test]
    fn rows_follow_the_column_then_the_name(tc: hegel::TestCase) {
        let commits = tc.draw(commit_list().print_as_debug());
        let by = tc.draw(any_column().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        let table = stats_table(&commits, false, by, reverse);
        let rows = table.as_ref().map_or(&[][..], StatsTable::rows);
        let ascending = (by == Column::Author) != reverse;
        for w in rows.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ord = column(a, b, by);
            let ord = if ascending { ord } else { ord.reverse() };
            assert!(
                ord.is_lt() || (ord.is_eq() && a.key < b.key),
                "{by:?} reverse={reverse}: {a:?} then {b:?}"
            );
        }
    }

    /// Sums saturate and `net` clamps each side to `i64::MAX`, so counts near `u64::MAX` neither
    /// panic nor wrap. The generator draws the full `u64` range, and the expected values are
    /// computed in wider integers.
    #[hegel::test]
    fn boundary_values_saturate_and_clamp(tc: hegel::TestCase) {
        let n = tc.draw(generators::integers::<usize>().min_value(1).max_value(20));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            let diff = DiffStat {
                insertions: tc.draw(generators::integers::<u64>()),
                deletions: tc.draw(generators::integers::<u64>()),
                files: tc.draw(generators::integers::<u64>()),
            };
            commits.push((authored_by(who("boundary", "b@x")), diff));
        }
        let table = stats_table(&commits, false, Column::Commits, false).unwrap();
        let saturated = |side: fn(&DiffStat) -> u64| {
            let sum: u128 = commits.iter().map(|(_, d)| u128::from(side(d))).sum();
            u64::try_from(sum).unwrap_or(u64::MAX)
        };
        let clamped = |n: u64| i128::from(n).min(i128::from(i64::MAX));
        let (ins, del) = (saturated(|d| d.insertions), saturated(|d| d.deletions));
        let total = table.total();
        assert_eq!((total.insertions, total.deletions), (ins, del));
        assert_eq!(total.files, saturated(|d| d.files));
        assert_eq!(i128::from(total.net()), clamped(ins) - clamped(del));
        // One Author, so the one row carries the same sums.
        assert_eq!(table.rows()[0].counts, total);
    }

    /// Each commit's Reviewers, drawn from a small pool, so ties in commit count are common, and a
    /// commit can name the same Reviewer twice. The pool has names that differ only in case.
    #[hegel::composite]
    fn reviewed_commits(tc: &hegel::TestCase) -> Vec<Vec<Contributor>> {
        const REVIEWERS: [(&str, &str); 4] = [
            ("Zed", "z@x"),
            ("Ada", "a@x"),
            ("Bob", "b@x"),
            ("ada", "l@x"),
        ];
        let n = tc.draw(generators::integers::<usize>().max_value(20));
        let mut commits = Vec::with_capacity(n);
        for _ in 0..n {
            let picks = tc.draw(
                generators::vecs(generators::integers::<usize>().max_value(REVIEWERS.len() - 1))
                    .max_size(3),
            );
            commits.push(
                picks
                    .into_iter()
                    .map(|i| who(REVIEWERS[i].0, REVIEWERS[i].1))
                    .collect(),
            );
        }
        commits
    }

    /// Each Reviewer's count is the number of commits that name them at least once.
    #[hegel::test]
    fn reviews_count_each_commit_once(tc: hegel::TestCase) {
        let commits = tc.draw(reviewed_commits().print_as_debug());
        let email = tc.draw(generators::booleans());
        let mut expected: BTreeMap<String, u64> = BTreeMap::new();
        for reviewers in &commits {
            let keys: BTreeSet<String> = reviewers.iter().map(|r| r.key(email)).collect();
            for key in keys {
                *expected.entry(key).or_default() += 1;
            }
        }
        let actual: BTreeMap<String, u64> = reviews_table(&commits, email)
            .into_iter()
            .map(|r| (r.key, r.commits))
            .collect();
        assert_eq!(actual, expected);
    }

    /// Reviewers come in descending commit count, and ties come A to Z. The order is strict, so it
    /// cannot depend on the order in which git or jj walks the commits.
    #[hegel::test]
    fn reviews_sort_by_count_then_name(tc: hegel::TestCase) {
        let commits = tc.draw(reviewed_commits().print_as_debug());
        let email = tc.draw(generators::booleans());
        for w in reviews_table(&commits, email).windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!(
                a.commits > b.commits || (a.commits == b.commits && a.key < b.key),
                "{a:?} then {b:?}"
            );
        }
    }
}
