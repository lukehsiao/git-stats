//! Helpers shared by the integration test crates.

// Each test crate compiles its own copy of this module and uses a different subset of it, so every
// crate would otherwise warn about the rest.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use git_stats::app;
use git_stats::logic::table::{Report, Review, Stat};
use git_stats::model::{Options, SortBy};
use git_stats::repo::Repo;

/// Default options over `range`, as the CLI gives them without flags.
pub fn options(range: Option<&str>) -> Options {
    Options {
        range: range.map(str::to_string),
        email: false,
        reviews: false,
        sort: SortBy::Commits,
        reverse: false,
        authors: Vec::new(),
        since: None,
        until: None,
    }
}

pub fn report(repo: &Repo, opts: &Options) -> Report {
    app::run(repo, opts).unwrap()
}

/// The stats table row whose Contributor key is exactly `key`.
pub fn author<'a>(report: &'a Report, key: &str) -> &'a Stat {
    report
        .stats
        .iter()
        .flat_map(|t| &t.rows)
        .find(|s| s.key == key)
        .unwrap_or_else(|| panic!("no author {key:?} in {report:#?}"))
}

/// Whether the stats table has a row whose Contributor key is exactly `key`.
pub fn has_author(report: &Report, key: &str) -> bool {
    report
        .stats
        .iter()
        .flat_map(|t| &t.rows)
        .any(|s| s.key == key)
}

pub fn total(report: &Report) -> &Stat {
    &report
        .stats
        .as_ref()
        .unwrap_or_else(|| panic!("no stats table in {report:#?}"))
        .total
}

/// A row's counts in table order: commits, files, insertions, deletions, net.
pub fn counts(s: &Stat) -> (u64, u64, u64, u64, i64) {
    (s.commits, s.num_files, s.insertions, s.deletions, s.net)
}

/// The reviews table row whose Contributor key is exactly `key`.
pub fn reviewer<'a>(report: &'a Report, key: &str) -> &'a Review {
    report
        .reviews
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no reviewer {key:?} in {report:#?}"))
}

/// Run git in `dir`, ignoring host config so commit creation is hermetic (no gpgsign, etc.), and
/// return its trimmed stdout.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git should be installed");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Whether a usable `git` binary is on PATH; tests skip gracefully without one.
pub fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}
