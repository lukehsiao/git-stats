use crate::model::{SortBy, Stat};

/// Sort per-author rows by the chosen column, descending by default. With
/// `reverse`, the column order is flipped to ascending. Ties sort by author
/// name, A to Z in both directions, so the order does not depend on which
/// commit the walk saw first. Authors are unique, as `aggregate` makes them,
/// so no two rows compare equal.
pub fn sort_stats(stats: &mut [Stat], sort: SortBy, reverse: bool) {
    stats.sort_by(|a, b| {
        let column = match sort {
            SortBy::Author => b.author.cmp(&a.author),
            SortBy::Commits => b.commits.cmp(&a.commits),
            SortBy::Files => b.num_files.cmp(&a.num_files),
            SortBy::Insertions => b.insertions.cmp(&a.insertions),
            SortBy::Deletions => b.deletions.cmp(&a.deletions),
            SortBy::Net => b.net.cmp(&a.net),
        };
        let column = if reverse { column.reverse() } else { column };
        column.then_with(|| a.author.cmp(&b.author))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hegel::generators::{self, Generator};

    #[hegel::composite]
    fn stat_list(tc: &hegel::TestCase) -> Vec<Stat> {
        let n = tc.draw(generators::integers::<usize>().max_value(100));
        let mut stats = Vec::with_capacity(n);
        for _ in 0..n {
            let who = tc.draw(generators::integers::<u8>().max_value(8));
            stats.push(Stat {
                // The space makes author sorting handle multi-word names.
                author: format!("Author {who}"),
                commits: u64::from(tc.draw(generators::integers::<u16>())),
                num_files: u64::from(tc.draw(generators::integers::<u16>())),
                insertions: u64::from(tc.draw(generators::integers::<u16>())),
                deletions: u64::from(tc.draw(generators::integers::<u16>())),
                net: i64::from(tc.draw(generators::integers::<i16>())),
            });
        }
        stats
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

    fn canonical(stats: &[Stat]) -> Vec<(String, u64, u64, u64, u64, i64)> {
        let mut key: Vec<_> = stats
            .iter()
            .map(|s| {
                (
                    s.author.clone(),
                    s.commits,
                    s.num_files,
                    s.insertions,
                    s.deletions,
                    s.net,
                )
            })
            .collect();
        key.sort();
        key
    }

    #[hegel::test]
    fn sort_preserves_the_multiset(tc: hegel::TestCase) {
        let original = tc.draw(stat_list().print_as_debug());
        let by = tc.draw(any_sort_by().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        let mut sorted = original.clone();
        sort_stats(&mut sorted, by, reverse);
        assert_eq!(canonical(&original), canonical(&sorted));
    }

    /// With unique authors, as `aggregate` produces them, the input order
    /// cannot change the output. git and jj walk commits in different orders.
    #[hegel::test]
    fn sort_is_independent_of_input_order(tc: hegel::TestCase) {
        let mut stats = tc.draw(stat_list().print_as_debug());
        let mut seen = std::collections::HashSet::new();
        stats.retain(|s| seen.insert(s.author.clone()));
        let by = tc.draw(any_sort_by().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        let mut forward = stats.clone();
        sort_stats(&mut forward, by, reverse);
        stats.reverse();
        sort_stats(&mut stats, by, reverse);
        assert_eq!(stats, forward);
    }

    /// How `a` compares to `b` in the column `by` names, ascending.
    fn column(a: &Stat, b: &Stat, by: SortBy) -> std::cmp::Ordering {
        match by {
            SortBy::Author => a.author.cmp(&b.author),
            SortBy::Commits => a.commits.cmp(&b.commits),
            SortBy::Files => a.num_files.cmp(&b.num_files),
            SortBy::Insertions => a.insertions.cmp(&b.insertions),
            SortBy::Deletions => a.deletions.cmp(&b.deletions),
            SortBy::Net => a.net.cmp(&b.net),
        }
    }

    /// The chosen column is non-increasing by default and non-decreasing
    /// with `reverse`.
    #[hegel::test]
    fn column_order_follows_the_direction(tc: hegel::TestCase) {
        let mut stats = tc.draw(stat_list().print_as_debug());
        let by = tc.draw(any_sort_by().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        sort_stats(&mut stats, by, reverse);
        let out_of_order = if reverse {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        };
        for w in stats.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert_ne!(
                column(a, b, by),
                out_of_order,
                "{by:?} reverse={reverse}: {a:?} then {b:?}"
            );
        }
    }

    /// Rows that tie in the chosen column are in author order, A to Z, in
    /// both directions.
    #[hegel::test]
    fn ties_are_in_author_order(tc: hegel::TestCase) {
        let mut stats = tc.draw(stat_list().print_as_debug());
        let by = tc.draw(any_sort_by().print_as_debug());
        let reverse = tc.draw(generators::booleans());
        sort_stats(&mut stats, by, reverse);
        for w in stats.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!(
                column(a, b, by).is_ne() || a.author <= b.author,
                "tie out of author order for {by:?}: {a:?} then {b:?}"
            );
        }
    }
}
