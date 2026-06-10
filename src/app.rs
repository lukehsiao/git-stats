//! Application: the thin coordinator. Follows the Logic Sandwich pattern,
//! reading from the repository, transforming with pure logic, and reading again
//! for numstats, then rendering. It owns no business rules of its own.

use crate::error::Result;
use crate::logic::{aggregate, filter, render, sort};
use crate::model::Options;
use crate::repo::{self, Repo, WalkedCommit};

/// Run the report and return its rendered text: the stats table followed by the
/// optional reviews table. Returns an empty string when there is nothing to show.
///
/// # Errors
///
/// Returns an error if the revision range cannot be resolved, a date or author
/// pattern is invalid, or a commit's diff cannot be read.
pub fn run(repo: &Repo, opts: &Options) -> Result<String> {
    // READ: walk the range once to get commit metadata.
    let walked = repo.walk(&opts.range, opts.reviews)?;

    let mut output = String::new();

    output.push_str(&stats_section(repo, opts, &walked)?);

    // Reviews intentionally use the full range, ignoring the author/date
    // filters, to match the original tool's behavior.
    if opts.reviews {
        let reviews = aggregate::aggregate_reviews(walked.iter().map(|w| &w.meta), opts.email);
        if !reviews.is_empty() {
            // The blank line only separates the tables; without a stats table
            // the reviews must start at the top.
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&render::render_reviews(&reviews));
            output.push('\n');
        }
    }

    Ok(output)
}

fn stats_section(repo: &Repo, opts: &Options, walked: &[WalkedCommit]) -> Result<String> {
    // PROCESS: author/date filter.
    let authors = filter::compile_authors(&opts.authors)?;
    let since = repo::parse_date(opts.since.as_deref())?;
    let until = repo::parse_date(opts.until.as_deref())?;
    let kept_idx = filter::keep_indices(walked.iter().map(|w| &w.meta), &authors, since, until);
    let kept: Vec<&WalkedCommit> = kept_idx.iter().map(|&i| &walked[i]).collect();

    // READ: numstat only the survivors.
    let diffs = repo.numstats(&kept)?;

    // PROCESS: aggregate, total, sort.
    let rows: Vec<aggregate::CommitStat> = kept
        .iter()
        .zip(diffs)
        .map(|(w, diff)| aggregate::CommitStat {
            author_key: aggregate::author_key(&w.meta.author, opts.email),
            diff,
        })
        .collect();
    let mut stats = aggregate::aggregate(&rows);
    if stats.is_empty() {
        return Ok(String::new());
    }
    sort::sort_stats(&mut stats, opts.sort, opts.reverse);
    stats.push(aggregate::compute_totals(&stats));

    let mut section = render::render_stats(&stats);
    section.push('\n');
    Ok(section)
}
