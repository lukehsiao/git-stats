use std::fmt::Write as _;

use unicode_width::UnicodeWidthStr;
use yansi::Paint;

use crate::logic::table::{Report, StatsTable};
use crate::model::Review;

/// Render the report as text: the stats table, then the reviews table after a blank line. A table
/// with nothing to show is left out, along with its blank line, so the text never opens with one.
/// Non-empty text ends with exactly one newline; an empty report renders as the empty string.
///
/// Blue comes from the ANSI 16 palette so terminal themes can remap it. Styling is gated by yansi's
/// global condition, so it is plain text when color is disabled.
#[must_use]
pub fn report(report: &Report) -> String {
    let mut out = String::new();
    if let Some(stats) = &report.stats {
        out.push_str(&render_stats(stats));
        out.push('\n');
    }
    if !report.reviews.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&render_reviews(&report.reviews));
        out.push('\n');
    }
    out
}

/// Render the per-author stats table, with the Total row last and in bold blue.
fn render_stats(stats: &StatsTable) -> String {
    const HEADER: [&str; 6] = [
        "Author",
        "Commits",
        "Changed Files",
        "Insertions",
        "Deletions",
        "Net Δ",
    ];
    let cells: Vec<Vec<String>> = stats
        .rows
        .iter()
        .chain(std::iter::once(&stats.total))
        .map(|r| {
            vec![
                r.author.clone(),
                r.commits.to_string(),
                r.num_files.to_string(),
                display_add(r.insertions),
                display_del(r.deletions),
                display_net(r.net),
            ]
        })
        .collect();
    // The Total row is emphasized to separate it from the per-author rows above it.
    table(&HEADER, &cells, Emphasis::HeaderAndLastRow)
}

/// Render the per-reviewer table. For consistency with the stats table, its header row is bold
/// blue.
fn render_reviews(rows: &[Review]) -> String {
    const HEADER: [&str; 2] = ["Reviewer/Tester", "Commits"];
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|r| vec![r.author.clone(), r.commits.to_string()])
        .collect();
    table(&HEADER, &cells, Emphasis::Header)
}

/// Which rows are rendered in bold blue.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Emphasis {
    Header,
    HeaderAndLastRow,
}

/// Lay out `header` above `cells` as a borderless table.
///
/// Every column is as wide as its widest entry, adjacent columns are separated by two spaces, and
/// the leading column is left-aligned while the numeric columns after it are right-aligned. Lines
/// carry no leading or trailing padding and the result has no trailing newline, so a caller decides
/// how the table is terminated.
fn table(header: &[&str], cells: &[Vec<String>], emphasis: Emphasis) -> String {
    let widths = column_widths(header, cells);
    let last_row = cells.len().wrapping_sub(1);
    let mut out = String::new();
    push_row(&mut out, header.iter().copied(), &widths, true);
    for (i, row) in cells.iter().enumerate() {
        out.push('\n');
        let bold = emphasis == Emphasis::HeaderAndLastRow && i == last_row;
        push_row(&mut out, row.iter().map(String::as_str), &widths, bold);
    }
    out
}

/// The display width of the widest entry in each column, headers included.
fn column_widths(header: &[&str], cells: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = header.iter().map(|h| h.width()).collect();
    for row in cells {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.width());
        }
    }
    widths
}

fn push_row<'a>(
    out: &mut String,
    cells: impl Iterator<Item = &'a str>,
    widths: &[usize],
    bold: bool,
) {
    let columns = widths.len();
    for (i, (cell, &width)) in cells.zip(widths).enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        // Measured rather than assumed, because a cell can exceed the column width only if it was
        // not among the entries the widths were taken from.
        let padding = width.saturating_sub(cell.width());
        if i == 0 {
            push_cell(out, cell, bold);
            // Padding the final column would only add invisible trailing space.
            if columns > 1 {
                out.extend(std::iter::repeat_n(' ', padding));
            }
        } else {
            out.extend(std::iter::repeat_n(' ', padding));
            push_cell(out, cell, bold);
        }
    }
}

fn push_cell(out: &mut String, cell: &str, bold: bool) {
    if bold {
        // Writing to a `String` cannot fail, so there is no error to handle.
        let _ = write!(out, "{}", cell.blue().bold());
    } else {
        out.push_str(cell);
    }
}

/// Format a deletion count: `0` stays `0`, otherwise it is shown as `-n`.
fn display_del(n: u64) -> String {
    match n {
        0 => "0".to_string(),
        n => format!("-{n}"),
    }
}

/// Format an insertion count: `0` stays `0`, otherwise it is shown as `+n`.
fn display_add(n: u64) -> String {
    match n {
        0 => "0".to_string(),
        n => format!("+{n}"),
    }
}

/// Format a net line delta: positive values get a leading `+`, zero and negative values render with
/// their natural sign (`0`, `-5`).
fn display_net(n: i64) -> String {
    if n > 0 {
        format!("+{n}")
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Stat;
    use hegel::generators::{self, Generator};

    fn stat(author: &str, net: i64) -> Stat {
        Stat {
            author: author.to_string(),
            commits: 1,
            num_files: 1,
            insertions: 1,
            deletions: 1,
            net,
        }
    }

    /// The pool mixes ASCII with wide CJK and a combining accent, because column widths are
    /// measured in terminal cells rather than bytes or `char`s.
    #[hegel::composite]
    fn stats_table(tc: &hegel::TestCase) -> StatsTable {
        const NAMES: [&str; 5] = ["Ada", "Ada Lovelace", "格蕾丝 <g@x>", "Jose\u{301}", ""];
        let draw_stat = |author: &str| Stat {
            author: author.to_string(),
            commits: tc.draw(generators::integers::<u64>()),
            num_files: tc.draw(generators::integers::<u64>()),
            insertions: tc.draw(generators::integers::<u64>()),
            deletions: tc.draw(generators::integers::<u64>()),
            net: tc.draw(generators::integers::<i64>()),
        };
        let n = tc.draw(generators::integers::<usize>().max_value(20));
        let mut rows = Vec::with_capacity(n);
        for _ in 0..n {
            let who = tc.draw(generators::integers::<usize>().max_value(NAMES.len() - 1));
            rows.push(draw_stat(NAMES[who]));
        }
        let total = draw_stat("Total");
        StatsTable { rows, total }
    }

    /// Columns only line up if every line occupies the same number of terminal cells, which is the
    /// whole job of the width measurement.
    #[hegel::test]
    fn every_stats_line_is_equally_wide(tc: hegel::TestCase) {
        yansi::disable();
        let out = render_stats(&tc.draw(stats_table().print_as_debug()));
        let widths: Vec<usize> = out.lines().map(UnicodeWidthStr::width).collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "line widths differ: {widths:?}\n{out}"
        );
    }

    #[hegel::test]
    fn stats_lines_never_end_in_whitespace(tc: hegel::TestCase) {
        yansi::disable();
        let out = render_stats(&tc.draw(stats_table().print_as_debug()));
        for line in out.lines() {
            assert_eq!(line.trim_end(), line, "trailing space in:\n{out}");
        }
    }

    #[test]
    fn columns_are_two_spaces_apart_with_numbers_right_aligned() {
        yansi::disable();
        let out = render_stats(&StatsTable {
            rows: vec![stat("Ada", -1)],
            total: stat("Total", -1),
        });
        assert_eq!(
            out,
            concat!(
                "Author  Commits  Changed Files  Insertions  Deletions  Net Δ\n",
                "Ada           1              1          +1         -1     -1\n",
                "Total         1              1          +1         -1     -1"
            )
        );
    }

    /// An author wider than its header widens the column for every other row.
    #[test]
    fn a_long_author_widens_the_first_column() {
        yansi::disable();
        let out = render_stats(&StatsTable {
            rows: vec![stat("Ada", 1), stat("Grace Brewster Hopper", 1)],
            total: stat("Total", 2),
        });
        // Every line's first column is padded to the widest author, so the second column starts at
        // the same offset on all of them.
        let width = "Grace Brewster Hopper".len();
        let labels = ["Author", "Ada", "Grace Brewster Hopper", "Total"];
        assert_eq!(out.lines().count(), labels.len(), "in:\n{out}");
        for (line, author) in out.lines().zip(labels) {
            assert_eq!(&line[..width], &format!("{author:<width$}"), "in:\n{out}");
        }
    }

    #[test]
    fn reviews_render_two_columns() {
        yansi::disable();
        let rows = vec![Review {
            author: "Ada".to_string(),
            commits: 3,
        }];
        assert_eq!(
            render_reviews(&rows),
            concat!("Reviewer/Tester  Commits\n", "Ada                    3")
        );
    }

    /// For any report, the text never opens with a blank line, and non-empty text ends with exactly
    /// one newline. A blank line separates the tables only when both show, so the reviews table
    /// alone starts at the top.
    #[hegel::test]
    fn report_framing_is_stable(tc: hegel::TestCase) {
        yansi::disable();
        let stats = if tc.draw(generators::booleans()) {
            Some(tc.draw(stats_table().print_as_debug()))
        } else {
            None
        };
        let n = tc.draw(generators::integers::<u64>().max_value(3));
        let reviews: Vec<Review> = (0..n)
            .map(|commits| Review {
                author: "Rev Iewer".to_string(),
                commits,
            })
            .collect();
        let both = stats.is_some() && !reviews.is_empty();
        let reviews_only = stats.is_none() && !reviews.is_empty();
        let out = report(&Report { stats, reviews });

        assert!(!out.starts_with('\n'), "report opens blank: {out:?}");
        if !out.is_empty() {
            assert!(
                out.ends_with('\n') && !out.ends_with("\n\n"),
                "report should end with exactly one newline: {out:?}"
            );
        }
        assert_eq!(out.matches("\n\n").count(), usize::from(both), "{out:?}");
        if reviews_only {
            assert!(out.starts_with("Reviewer/Tester"), "{out:?}");
        }
    }

    #[test]
    fn an_empty_report_renders_nothing() {
        let empty = Report {
            stats: None,
            reviews: Vec::new(),
        };
        assert_eq!(report(&empty), "");
    }

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

    /// Across the full `i64` range (including `MIN`, `MAX`, `0`), `display_net` never panics and
    /// follows its sign rules.
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
