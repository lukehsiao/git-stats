//! A narrow integration test against a real, throwaway git repository. It
//! guards against drift between the nulled [`Repo`] used elsewhere and the
//! actual gix-backed implementation (range walking, numstat, trailers).

use std::fmt::Write as _;
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
    // Control for the shallow-clone probe: a full repository must not warn.
    assert!(!repo.is_shallow(), "full repository detected as shallow");
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

/// Three commits on main with both tag flavors on the middle commit: the shape
/// `git describe`-driven release ranges produce. The tempdir is returned so the
/// repository outlives the test body.
fn tag_fixture() -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    for name in ["a", "b", "c"] {
        std::fs::write(p.join(name).with_extension("txt"), "x\n").unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "-m", name]);
    }
    git(p, &["tag", "-a", "-m", "release", "annotated", "HEAD~1"]);
    git(p, &["tag", "lightweight", "HEAD~1"]);

    let repo = Repo::open(p).unwrap();
    (dir, repo)
}

fn total_commits(out: &str) -> Option<&str> {
    row(out, "Total").split_whitespace().nth(1)
}

#[test]
fn lightweight_tag_range_is_unaffected() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (_dir, repo) = tag_fixture();
    let out = report(&repo, &options("lightweight..HEAD", false));

    assert_eq!(
        total_commits(&out),
        Some("1"),
        "lightweight..HEAD should count only the commit after the tag:\n{out}"
    );
}

#[test]
fn annotated_tag_range_excludes_commits_behind_the_tag() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (_dir, repo) = tag_fixture();
    let out = report(&repo, &options("annotated..HEAD", false));

    // The tag object must be peeled to its commit before going into the hidden
    // set; an unpeeled tag OID matches nothing and the range silently degrades
    // to whole-repo history (3 commits here).
    assert_eq!(
        total_commits(&out),
        Some("1"),
        "annotated..HEAD should count only the commit after the tag:\n{out}"
    );
}

#[test]
fn annotated_tag_on_the_inclusion_side_resolves() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (_dir, repo) = tag_fixture();
    let out = report(&repo, &options("HEAD~2..annotated", false));

    assert_eq!(
        total_commits(&out),
        Some("1"),
        "HEAD~2..annotated should count only the tagged commit:\n{out}"
    );
}

#[test]
fn a_bare_annotated_tag_resolves_to_its_target_commit() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (dir, repo) = tag_fixture();
    let out = report(&repo, &options("annotated", false));

    // The tag points at the second of three commits, so its history is 2 deep.
    assert_eq!(
        total_commits(&out),
        Some("2"),
        "a bare annotated tag should walk from its target commit:\n{out}"
    );

    // git peels tags recursively, so a tag pointing at another tag must
    // resolve to the same commit.
    git(
        dir.path(),
        &["tag", "-a", "-m", "wrap", "nested", "annotated"],
    );
    let out = report(&repo, &options("nested", false));
    assert_eq!(
        total_commits(&out),
        Some("2"),
        "a tag-to-tag chain should peel to the same commit:\n{out}"
    );
}

#[test]
fn symmetric_difference_with_an_annotated_tag_resolves() {
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
    git(p, &["commit", "-q", "-m", "feature only"]);
    git(p, &["tag", "-a", "-m", "release", "feature-tag"]);

    git(p, &["checkout", "-q", "main"]);
    std::fs::write(p.join("c.txt"), "c\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "main only"]);

    let repo = Repo::open(p).unwrap();
    // Unlike the two-dot tests, `...` also feeds the endpoints to the
    // merge-base computation, which needs commit OIDs rather than tag OIDs.
    let out = report(&repo, &options("main...feature-tag", false));

    assert_eq!(
        total_commits(&out),
        Some("2"),
        "main...feature-tag should count the 2 divergent commits:\n{out}"
    );
}

#[test]
fn binary_file_changes_count_as_changed_files() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    // One text file and one binary file (NUL bytes) in the same commit.
    // `git log --numstat` lists the binary file as `-  -  path` and
    // `git diff --shortstat` reports "2 files changed, 2 insertions(+)".
    std::fs::write(p.join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(p.join("blob.bin"), [0u8, 159, 146, 150, 0, 10]).unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "add text and binary"]);

    let repo = Repo::open(p).unwrap();
    let out = report(&repo, &options("HEAD", false));

    let cols: Vec<&str> = row(&out, "Total").split_whitespace().collect();
    assert_eq!(
        cols[2], "2",
        "the binary file should count as a changed file:\n{out}"
    );
    assert_eq!(
        cols[3], "+2",
        "only the text file should contribute lines:\n{out}"
    );
}

#[test]
fn shallow_clones_treat_boundary_commits_as_parentless() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let origin = p.join("origin");
    std::fs::create_dir(&origin).unwrap();
    git(&origin, &["init", "-q", "-b", "main"]);
    git(&origin, &["config", "user.name", "Ada"]);
    git(&origin, &["config", "user.email", "ada@example.com"]);

    // Three commits, each growing the same file by one line.
    let mut content = String::new();
    for i in 1..=3 {
        writeln!(content, "line{i}").unwrap();
        std::fs::write(origin.join("f.txt"), &content).unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-q", "-m", &format!("c{i}")]);
    }

    // A depth-2 clone keeps c2 and c3, with c2 as the shallow boundary: its
    // parent c1 exists in c2's header but not in the object database. The
    // file:// URL matters; a bare local path would not produce a shallow clone.
    let url = format!("file://{}", origin.display());
    git(p, &["clone", "-q", "--depth", "2", &url, "shallow"]);

    let repo = Repo::open(p.join("shallow")).unwrap();
    // The binary warns on shallow clones via this probe; the full-clone
    // control lives in `reads_a_real_repository`.
    assert!(repo.is_shallow(), "depth-2 clone should detect as shallow");
    let out = report(&repo, &options("HEAD", false));

    // git log --numstat here shows c3 as +1 and the boundary commit c2 as its
    // whole tree (+2), exactly like a root commit, so the total is +3.
    let cols: Vec<&str> = row(&out, "Total").split_whitespace().collect();
    assert_eq!(cols[1], "2", "both retained commits should count:\n{out}");
    assert_eq!(
        cols[3], "+3",
        "the boundary commit should diff against the empty tree:\n{out}"
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
