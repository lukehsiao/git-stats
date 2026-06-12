use tabled::{
    Table,
    settings::{
        Alignment, Padding, Style,
        format::Format,
        object::{Columns, Rows},
    },
};
use yansi::Paint;

use crate::model::{Review, Stat};

/// Render the per-author stats table.
///
/// Blue comes from the ANSI 16 palette so terminal themes can remap it. Styling
/// is gated by yansi's global condition, so it is plain text when color is
/// disabled.
#[must_use]
pub fn render_stats(rows: &[Stat]) -> String {
    let mut table = Table::new(rows);
    table
        .with(Style::empty())
        .modify(Columns::first(), Padding::new(0, 1, 0, 0))
        .modify(Columns::last(), Padding::new(1, 0, 0, 0))
        .modify(Columns::new(1..=5), Alignment::right())
        .modify(
            Rows::first(),
            Format::content(|s| s.blue().bold().to_string()),
        )
        .modify(
            Rows::last(),
            Format::content(|s| s.blue().bold().to_string()),
        );
    table.to_string()
}

/// Render the per-reviewer table. For consistency with the stats table, its
/// header row is bold blue (gated by yansi's global color condition).
#[must_use]
pub fn render_reviews(rows: &[Review]) -> String {
    let mut table = Table::new(rows);
    table
        .with(Style::empty())
        .modify(Columns::first(), Padding::new(0, 1, 0, 0))
        .modify(Columns::last(), Padding::new(1, 0, 0, 0))
        .modify(Columns::new(1..=1), Alignment::right())
        .modify(
            Rows::first(),
            Format::content(|s| s.blue().bold().to_string()),
        );
    table.to_string()
}
