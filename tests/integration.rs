//! A narrow integration test against a real, throwaway git repository. It
//! guards against drift between the nulled [`Repo`] used elsewhere and the
//! actual gix-backed implementation (range walking, numstat, trailers).

use std::path::Path;
use std::process::Command;

use git_stats::app;
use git_stats::model::{Options, SortBy};
use git_stats::repo::Repo;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        // Ignore host config so commit creation is hermetic (no gpgsign, etc.).
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .expect("git should be installed");
    assert!(status.success(), "git {args:?} failed");
}

fn options(range: &str, reviews: bool) -> Options {
    Options {
        range: range.to_string(),
        email: false,
        reviews,
        sort: SortBy::Commits,
        reverse: false,
        authors: Vec::new(),
        since: None,
        until: None,
    }
}

fn row<'a>(out: &'a str, label: &str) -> &'a str {
    out.lines()
        .find(|l| l.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("no row starting with {label:?} in:\n{out}"))
}

/// Run with color disabled so assertions see plain text.
fn report(repo: &Repo, opts: &Options) -> String {
    yansi::disable();
    app::run(repo, opts).unwrap()
}

/// Whether a usable `git` binary is on PATH; tests skip gracefully without one.
fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[test]
fn reads_a_real_repository() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    std::fs::write(p.join("a.txt"), "line1\nline2\n").unwrap();
    git(p, &["add", "."]);
    git(
        p,
        &[
            "commit",
            "-q",
            "-m",
            "first",
            "-m",
            "Reviewed-by: Rev Iewer <rev@example.com>",
        ],
    );

    std::fs::write(p.join("a.txt"), "line1\nline2\nline3\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "second"]);

    let repo = Repo::open(p).unwrap();
    let out = report(&repo, &options("HEAD", true));

    // Ada: 2 commits; root adds 2 lines, the follow-up adds 1, so +3 insertions.
    let ada = row(&out, "Ada");
    assert!(ada.contains("+3"), "ada row: {ada}");
    // The Reviewed-by trailer surfaces in the reviews table.
    assert!(out.contains("Rev Iewer"), "reviews missing:\n{out}");
}

#[test]
fn range_excludes_commits_on_the_excluded_side() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    // One commit on main.
    std::fs::write(p.join("a.txt"), "a\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "base on main"]);

    // Two more commits on a feature branch.
    git(p, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "feature 1"]);
    std::fs::write(p.join("c.txt"), "c\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "feature 2"]);

    let repo = Repo::open(p).unwrap();
    let out = report(&repo, &options("main..feature", false));

    // `main..feature` excludes the base commit, leaving exactly the 2 feature commits.
    let total = row(&out, "Total");
    assert_eq!(
        total.split_whitespace().nth(1),
        Some("2"),
        "main..feature should count only the 2 feature commits:\n{out}"
    );
}

#[test]
fn symmetric_difference_range_excludes_the_common_ancestor() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    // Common base on main.
    std::fs::write(p.join("a.txt"), "a\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "base"]);

    // One commit only on feature.
    git(p, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "feature only"]);

    // One commit only on main, so the branches diverge.
    git(p, &["checkout", "-q", "main"]);
    std::fs::write(p.join("c.txt"), "c\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "main only"]);

    let repo = Repo::open(p).unwrap();
    let out = report(&repo, &options("main...feature", false));

    // The symmetric difference drops the shared base, leaving the 2 divergent commits.
    let total = row(&out, "Total");
    assert_eq!(
        total.split_whitespace().nth(1),
        Some("2"),
        "main...feature should count the 2 divergent commits:\n{out}"
    );
}

#[test]
fn merge_commits_are_counted_but_add_no_lines() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    std::fs::write(p.join("a.txt"), "a\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "base"]);

    git(p, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "feature"]);

    // Diverge main so merging cannot fast-forward, forcing a real merge commit.
    git(p, &["checkout", "-q", "main"]);
    std::fs::write(p.join("c.txt"), "c\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "main"]);
    git(p, &["merge", "--no-ff", "--no-edit", "feature"]);

    let repo = Repo::open(p).unwrap();
    let out = report(&repo, &options("HEAD", false));

    // Four commits (base, feature, main, merge); the merge contributes no lines,
    // so insertions total only the three single-line file additions.
    let cols: Vec<&str> = row(&out, "Total").split_whitespace().collect();
    assert_eq!(cols[1], "4", "merge commit should be counted:\n{out}");
    assert_eq!(
        cols[3], "+3",
        "merge should contribute no insertions:\n{out}"
    );
}
