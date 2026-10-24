//! Integration tests that need the `git` binary: they compare against git's own output, read what
//! a real `git clone --depth` or a corrupt commit-graph leaves behind, or run the git-stats binary
//! itself. Everything else builds its repository with `common::RepoBuilder`.

use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};

use git_stats::repo::{Repo, Vcs};

mod common;
use common::{git, git_available, read, report, reviewer, run, sum};

/// `git stats | head` style usage closes stdout before the report is fully written. Rust ignores
/// SIGPIPE, so a plain `print!` panics on the resulting EPIPE; the binary must instead die quietly
/// with the same status a shell reports when git itself is killed by SIGPIPE (128 + 13).
#[test]
fn broken_pipe_exits_quietly() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);

    // Enough single-commit authors that the rendered table exceeds the pipe buffer (64 KiB), so the
    // write blocks until the reader goes away rather than racing the reader's exit. Empty commits
    // keep fast-import instant.
    let mut input = String::new();
    for i in 0..2000 {
        let msg = format!("c{i}");
        write!(
            input,
            "commit refs/heads/main\ncommitter Author Number {i:04} \
             <author{i:04}@example.com> {} +0000\ndata {}\n{msg}\n",
            1_600_000_000 + i,
            msg.len(),
        )
        .unwrap();
    }
    let mut fast_import = Command::new("git")
        .current_dir(p)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = fast_import.stdin.take().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    assert!(fast_import.wait().unwrap().success(), "fast-import failed");

    let mut child = Command::new(env!("CARGO_BIN_EXE_git-stats"))
        .current_dir(p)
        .arg("--email")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Close the read end without reading anything; the blocked write sees EPIPE.
    drop(child.stdout.take());
    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();

    assert!(
        !stderr.contains("panicked"),
        "panicked on closed stdout:\n{stderr}"
    );
    assert_eq!(
        status.code(),
        Some(141),
        "expected git-style SIGPIPE exit, stderr:\n{stderr}"
    );
}

/// git honors `GIT_DIR` when locating the repository, and sets it itself when running hooks, so
/// `GIT_DIR=... git stats` from outside the repository must work exactly like `GIT_DIR=... git log`
/// does.
#[test]
fn git_dir_environment_override_is_honored() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let repo_dir = p.join("repo");
    std::fs::create_dir(&repo_dir).unwrap();
    git(&repo_dir, &["init", "-q", "-b", "main"]);
    git(&repo_dir, &["config", "user.name", "Ada"]);
    git(&repo_dir, &["config", "user.email", "ada@example.com"]);
    std::fs::write(repo_dir.join("a.txt"), "a\n").unwrap();
    git(&repo_dir, &["add", "."]);
    git(&repo_dir, &["commit", "-q", "-m", "c1"]);

    // A directory that is not inside any repository.
    let elsewhere = p.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_git-stats"))
        .current_dir(&elsewhere)
        .env("GIT_DIR", repo_dir.join(".git"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "GIT_DIR run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("Ada"),
        "report should show the GIT_DIR repository's author:\n{stdout}"
    );
}

/// Reviewers resolve through `.mailmap` and split `Name <email>` exactly as `git shortlog
/// --group=trailer` does, so both tools credit the same rows with the same counts.
#[test]
fn reviewers_match_git_shortlog_by_trailer() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);
    std::fs::write(
        p.join(".mailmap"),
        "New Name <new@example.com> <old@example.com>\n",
    )
    .unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "add mailmap"]);
    let reviewed_by = [
        "Old Name <old@example.com>",
        "New Name <new@example.com>",
        // git splits at the first `>` after the `<`, so the comment is not part of the email.
        "Rev <rev@example.com> # see <foo>",
        // git trims only ASCII whitespace before the `<`, so the no-break space stays in the name.
        "Nbsp\u{a0} <nbsp@example.com>",
        // Not an identity, so git takes it literally.
        "Some Team",
    ];
    for value in reviewed_by {
        git(
            p,
            &[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "change",
                "-m",
                &format!("Reviewed-by: {value}"),
            ],
        );
    }

    let shortlog = git(
        p,
        &["shortlog", "-sne", "--group=trailer:reviewed-by", "HEAD"],
    );
    let mut want: Vec<(u64, String)> = shortlog
        .lines()
        .map(|l| {
            let (count, key) = l.trim().split_once('\t').unwrap();
            (count.parse().unwrap(), key.to_string())
        })
        .collect();
    want.sort();

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let r = report(&repo, &["--email", "--reviews", "HEAD"]);
    let mut got: Vec<(u64, String)> = r
        .reviews
        .iter()
        .map(|rv| (rv.commits, rv.key.clone()))
        .collect();
    got.sort();

    assert_eq!(got, want, "git shortlog:\n{shortlog}\n{r:#?}");
    assert_eq!(reviewer(&r, "New Name <new@example.com>").commits, 2);
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

    // A depth-2 clone keeps c2 and c3, with c2 as the shallow boundary: its parent c1 exists in
    // c2's header but not in the object database. The file:// URL matters; a bare local path would
    // not produce a shallow clone.
    let url = format!("file://{}", origin.display());
    git(p, &["clone", "-q", "--depth", "2", &url, "shallow"]);

    let repo = Repo::open(p.join("shallow"), Vcs::Git).unwrap();
    // The binary warns on shallow clones via this probe; the full-clone control lives in
    // `tests/repo.rs`, `counts_lines_and_review_trailers`.
    assert!(repo.is_shallow(), "depth-2 clone should detect as shallow");
    let (n, diff) = sum(&read(&repo, Some("HEAD")));

    // git log --numstat here shows c3 as +1 and the boundary commit c2 as its whole tree (+2),
    // exactly like a root commit, so the total is +3.
    assert_eq!(n, 2, "both retained commits should count");
    assert_eq!(
        diff.insertions, 3,
        "the boundary commit should diff against the empty tree"
    );
}

/// A commit whose header git itself would never write (non-numeric committer timestamp) fails
/// gitoxide's strict decode. Tolerating such fsck-level corruption is out of scope, but the error
/// must name the offending commit; an anonymous "object parsing failed" is undiagnosable in a large
/// history.
///
/// The named path requires a commit-graph (gc writes one by default since git 2.24): with it,
/// traversal reads parents from the graph and our decode is the first to touch the corrupt object.
/// Without it, gitoxide's walk iterator fails first and its error carries no id, which only
/// gitoxide can fix upstream.
#[test]
fn decode_failures_name_the_offending_commit() {
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
    git(p, &["commit", "-q", "-m", "good"]);

    let tree = git(p, &["rev-parse", "HEAD^{tree}"]);
    let parent = git(p, &["rev-parse", "HEAD"]);
    let raw = format!(
        "tree {tree}\nparent {parent}\nauthor Ada <ada@x> not-a-timestamp\n\
         committer Ada <ada@x> not-a-timestamp\n\nbad date\n"
    );
    std::fs::write(p.join("raw-commit"), &raw).unwrap();
    let bad = git(
        p,
        &[
            "hash-object",
            "--literally",
            "-t",
            "commit",
            "-w",
            "raw-commit",
        ],
    );
    git(p, &["update-ref", "refs/heads/main", &bad]);
    git(p, &["commit-graph", "write", "--reachable"]);

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let err = run(&repo, &["HEAD"]).unwrap_err();
    assert!(
        err.to_string().contains(&bad),
        "error should name the undecodable commit {bad}: {err}"
    );
}
