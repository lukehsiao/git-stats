//! Application: the thin coordinator. Follows the Logic Sandwich pattern, reading from the
//! repository, transforming with pure logic, and reading again for numstats. It returns the report
//! as data; rendering it is the caller's choice.

use regex::Regex;

use crate::error::Result;
use crate::logic::filter;
use crate::logic::table::{self, Report, StatsTable};
use crate::model::Options;
use crate::repo::{self, Repo, WalkedCommit};

/// Build the report: the stats table, and the reviews table when `opts.reviews` is set.
///
/// # Errors
///
/// Returns an error if the revision range cannot be resolved, a date or author
/// pattern is invalid, or a commit's diff cannot be read.
pub fn run(repo: &Repo, opts: &Options) -> Result<Report> {
    // PROCESS: validate the filters first. They are cheap to compile, and a
    // typo'd pattern or date should error immediately, not after a walk that
    // can take minutes on a large history.
    let authors = filter::compile_authors(&opts.authors)?;
    let since = repo::parse_date(opts.since.as_deref())?;
    let until = repo::parse_date(opts.until.as_deref())?;

    // READ: walk the range once to get commit metadata.
    let walked = repo.walk(opts.range.as_deref(), opts.reviews)?;

    let stats = stats_table(repo, opts, &authors, since, until, &walked)?;

    // Reviews intentionally use the full range, ignoring the author/date
    // filters, to match the original tool's behavior.
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
    authors: &[Regex],
    since: Option<i64>,
    until: Option<i64>,
    walked: &[WalkedCommit],
) -> Result<Option<StatsTable>> {
    // PROCESS: author/date filter.
    let kept_idx = filter::keep_indices(walked.iter().map(|w| &w.meta), authors, since, until);
    let kept: Vec<&WalkedCommit> = kept_idx.iter().map(|&i| &walked[i]).collect();

    // READ: numstat only the survivors.
    let diffs = repo.numstats(&kept)?;

    // PROCESS: build the table.
    let rows: Vec<table::CommitStat> = kept
        .iter()
        .zip(diffs)
        .map(|(w, diff)| table::CommitStat {
            author_key: table::author_key(&w.meta.author, opts.email),
            diff,
        })
        .collect();
    Ok(table::stats_table(&rows, opts.sort, opts.reverse))
}
