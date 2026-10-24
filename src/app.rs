//! Application: the thin coordinator. It validates the filters, reads the Revision range through
//! the repository with those filters, and builds the tables. It returns the report as data;
//! rendering it is the caller's choice.

use gix::date::Zoned;

use crate::error::Result;
use crate::logic::filter::CommitFilter;
use crate::logic::table::{self, Report};
use crate::model::Options;
use crate::repo::Repo;

/// Build the report: the stats table, and the reviews table when `opts.reviews` is set. Relative
/// `--since` and `--until` dates resolve against `now`.
///
/// # Errors
///
/// Returns an error if the revision range cannot be resolved, a date or author pattern is invalid,
/// or a commit's diff cannot be read.
pub fn run(repo: &Repo, opts: &Options, now: &Zoned) -> Result<Report> {
    // Validate the filters first. They are cheap to compile, and a typo'd pattern or date should
    // error immediately, not after a walk that can take minutes on a large history.
    let filter = CommitFilter::new(
        &opts.authors,
        opts.since.as_deref(),
        opts.until.as_deref(),
        now,
    )?;
    let commits = repo.read(opts.range.as_deref(), |m| filter.keeps(m), opts.reviews)?;
    let stats = table::stats_table(&commits.kept, opts.email, opts.sort, opts.reverse);
    // Reviews cover the full range, ignoring the author and date filters, to match the original
    // tool's behavior.
    let reviewers = commits.reviewers.as_deref().unwrap_or_default();
    let reviews = table::reviews_table(reviewers, opts.email);
    Ok(Report { stats, reviews })
}
