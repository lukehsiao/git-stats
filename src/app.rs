//! Application: the thin coordinator. Follows the Logic Sandwich pattern, reading from the
//! repository, transforming with pure logic, and reading again for numstats. It returns the report
//! as data; rendering it is the caller's choice.

use gix::date::Zoned;

use crate::error::Result;
use crate::logic::filter::CommitFilter;
use crate::logic::table::{self, Report, StatsTable};
use crate::model::Options;
use crate::repo::{Repo, WalkedCommit};

/// Build the report: the stats table, and the reviews table when `opts.reviews` is set. Relative
/// `--since` and `--until` dates resolve against `now`.
///
/// # Errors
///
/// Returns an error if the revision range cannot be resolved, a date or author pattern is invalid,
/// or a commit's diff cannot be read.
pub fn run(repo: &Repo, opts: &Options, now: &Zoned) -> Result<Report> {
    // PROCESS: validate the filters first. They are cheap to compile, and a typo'd pattern or date
    // should error immediately, not after a walk that can take minutes on a large history.
    let filter = CommitFilter::new(
        &opts.authors,
        opts.since.as_deref(),
        opts.until.as_deref(),
        now,
    )?;

    // READ: walk the range once to get commit metadata.
    let walked = repo.walk(opts.range.as_deref(), opts.reviews)?;

    let stats = stats_table(repo, opts, &filter, &walked)?;

    // Reviews intentionally use the full range, ignoring the author/date filters, to match the
    // original tool's behavior.
    let reviews = if opts.reviews {
        table::reviews_table(walked.iter().map(|w| &w.meta), opts.email)
    } else {
        Vec::new()
    };

    Ok(Report { stats, reviews })
}

fn stats_table(
    repo: &Repo,
    opts: &Options,
    filter: &CommitFilter,
    walked: &[WalkedCommit],
) -> Result<Option<StatsTable>> {
    // PROCESS: author/date filter.
    let kept: Vec<&WalkedCommit> = walked.iter().filter(|w| filter.keeps(&w.meta)).collect();

    // READ: numstat only the survivors.
    let commits = repo.numstats(&kept)?;

    // PROCESS: build the table.
    Ok(table::stats_table(
        commits,
        opts.email,
        opts.sort,
        opts.reverse,
    ))
}
