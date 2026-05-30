use std::cmp::Reverse;

use crate::model::{SortBy, Stat};

/// Sort per-author rows by the chosen column, descending by default. With
/// `reverse`, the order is flipped to ascending. The sort is stable, so ties
/// keep their relative input order.
pub fn sort_stats(stats: &mut [Stat], sort: SortBy, reverse: bool) {
    match sort {
        SortBy::Author => stats.sort_by(|a, b| b.author.cmp(&a.author)),
        SortBy::Commits => stats.sort_by_key(|s| Reverse(s.commits)),
        SortBy::Files => stats.sort_by_key(|s| Reverse(s.num_files)),
        SortBy::Insertions => stats.sort_by_key(|s| Reverse(s.insertions)),
        SortBy::Deletions => stats.sort_by_key(|s| Reverse(s.deletions)),
        SortBy::Net => stats.sort_by_key(|s| Reverse(s.net)),
    }
    if reverse {
        stats.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hegel::generators::{self, Generator};

    #[hegel::composite]
    fn stat_list(tc: hegel::TestCase) -> Vec<Stat> {
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
        let original = tc.draw(stat_list());
        let by = tc.draw(any_sort_by());
        let reverse = tc.draw(generators::booleans());
        let mut sorted = original.clone();
        sort_stats(&mut sorted, by, reverse);
        assert_eq!(canonical(&original), canonical(&sorted));
    }

    /// The default (non-reversed) order is non-increasing in the chosen column.
    #[hegel::test]
    fn default_order_is_descending(tc: hegel::TestCase) {
        let mut stats = tc.draw(stat_list());
        let by = tc.draw(any_sort_by());
        sort_stats(&mut stats, by, false);
        for w in stats.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let descending = match by {
                SortBy::Author => a.author >= b.author,
                SortBy::Commits => a.commits >= b.commits,
                SortBy::Files => a.num_files >= b.num_files,
                SortBy::Insertions => a.insertions >= b.insertions,
                SortBy::Deletions => a.deletions >= b.deletions,
                SortBy::Net => a.net >= b.net,
            };
            assert!(descending, "not descending for {by:?}: {a:?} then {b:?}");
        }
    }
}
