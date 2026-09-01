use std::fmt::Write as _;

use unicode_width::UnicodeWidthStr;
use yansi::Paint;

use crate::model::{Review, Stat, display_add, display_del, display_net};

/// Render the per-author stats table.
///
/// Blue comes from the ANSI 16 palette so terminal themes can remap it. Styling
/// is gated by yansi's global condition, so it is plain text when color is
/// disabled.
#[must_use]
pub fn render_stats(rows: &[Stat]) -> String {
    const HEADER: [&str; 6] = [
        "Author",
        "Commits",
        "Changed Files",
        "Insertions",
        "Deletions",
        "Net Δ",
    ];
    let cells: Vec<Vec<String>> = rows
        .iter()
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
    // The caller appends the totals as the final row, which is emphasized to
    // separate it from the per-author rows above it.
    table(&HEADER, &cells, Emphasis::HeaderAndLastRow)
}

/// Render the per-reviewer table. For consistency with the stats table, its
/// header row is bold blue (gated by yansi's global color condition).
#[must_use]
pub fn render_reviews(rows: &[Review]) -> String {
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
/// Every column is as wide as its widest entry, adjacent columns are separated
/// by two spaces, and the leading column is left-aligned while the numeric
/// columns after it are right-aligned. Lines carry no leading or trailing
/// padding and the result has no trailing newline, so a caller decides how the
/// table is terminated.
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
        // Measured rather than assumed, because a cell can exceed the column
        // width only if it was not among the entries the widths were taken from.
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

#[cfg(test)]
mod tests {
    use super::*;
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

    /// The pool mixes ASCII with wide CJK and a combining accent, because column
    /// widths are measured in terminal cells rather than bytes or `char`s.
    #[hegel::composite]
    fn stat_list(tc: &hegel::TestCase) -> Vec<Stat> {
        const NAMES: [&str; 5] = ["Ada", "Ada Lovelace", "格蕾丝 <g@x>", "Jose\u{301}", ""];
        let n = tc.draw(generators::integers::<usize>().max_value(20));
        let mut rows = Vec::with_capacity(n);
        for _ in 0..n {
            let who = tc.draw(generators::integers::<usize>().max_value(NAMES.len() - 1));
            rows.push(Stat {
                author: NAMES[who].to_string(),
                commits: tc.draw(generators::integers::<u64>()),
                num_files: tc.draw(generators::integers::<u64>()),
                insertions: tc.draw(generators::integers::<u64>()),
                deletions: tc.draw(generators::integers::<u64>()),
                net: tc.draw(generators::integers::<i64>()),
            });
        }
        rows
    }

    /// Columns only line up if every line occupies the same number of terminal
    /// cells, which is the whole job of the width measurement.
    #[hegel::test]
    fn every_stats_line_is_equally_wide(tc: hegel::TestCase) {
        yansi::disable();
        let out = render_stats(&tc.draw(stat_list().print_as_debug()));
        let widths: Vec<usize> = out.lines().map(UnicodeWidthStr::width).collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "line widths differ: {widths:?}\n{out}"
        );
    }

    #[hegel::test]
    fn stats_lines_never_end_in_whitespace(tc: hegel::TestCase) {
        yansi::disable();
        let out = render_stats(&tc.draw(stat_list().print_as_debug()));
        for line in out.lines() {
            assert_eq!(line.trim_end(), line, "trailing space in:\n{out}");
        }
    }

    #[test]
    fn columns_are_two_spaces_apart_with_numbers_right_aligned() {
        yansi::disable();
        let out = render_stats(&[stat("Ada", -1)]);
        assert_eq!(
            out,
            concat!(
                "Author  Commits  Changed Files  Insertions  Deletions  Net Δ\n",
                "Ada           1              1          +1         -1     -1"
            )
        );
    }

    /// An author wider than its header widens the column for every other row.
    #[test]
    fn a_long_author_widens_the_first_column() {
        yansi::disable();
        let out = render_stats(&[stat("Ada", 1), stat("Grace Brewster Hopper", 1)]);
        // Every line's first column is padded to the widest author, so the
        // second column starts at the same offset on all of them.
        let width = "Grace Brewster Hopper".len();
        for (line, author) in out.lines().zip(["Author", "Ada", "Grace Brewster Hopper"]) {
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
}
