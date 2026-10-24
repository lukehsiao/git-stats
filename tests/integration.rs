//! A narrow integration test against a real, throwaway git repository. It guards against drift
//! between the nulled [`Repo`] used elsewhere and the actual gix-backed implementation (range
//! walking, numstat, trailers).

use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};

use git_stats::app;
use git_stats::model::Vcs;
use git_stats::repo::Repo;

mod common;
use common::{author, counts, git, git_available, options, report, reviewer, total};

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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    // Control for the shallow-clone probe: a full repository must not warn.
    assert!(!repo.is_shallow(), "full repository detected as shallow");
    let mut opts = options(Some("HEAD"));
    opts.reviews = true;
    let r = report(&repo, &opts);

    // Ada: 2 commits; root adds 2 lines, the follow-up adds 1, so +3 insertions.
    assert_eq!(author(&r, "Ada").insertions, 3, "{r:#?}");
    // The Reviewed-by trailer surfaces in the reviews table.
    assert_eq!(reviewer(&r, "Rev Iewer").commits, 1, "{r:#?}");
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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("main..feature")));

    // `main..feature` excludes the base commit, leaving exactly the 2 feature commits.
    assert_eq!(
        total(&r).commits,
        2,
        "main..feature should count only the 2 feature commits:\n{r:#?}"
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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("main...feature")));

    // The symmetric difference drops the shared base, leaving the 2 divergent commits.
    assert_eq!(
        total(&r).commits,
        2,
        "main...feature should count the 2 divergent commits:\n{r:#?}"
    );
}

#[test]
fn symmetric_difference_hides_every_merge_base() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Ada"]);
    git(p, &["config", "user.email", "ada@example.com"]);

    // Criss-cross history: each branch merges the other's first commit, so br-a...br-b has two
    // merge bases (A and B), not one.
    std::fs::write(p.join("base.txt"), "base\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "base"]);

    git(p, &["checkout", "-q", "-b", "br-a"]);
    std::fs::write(p.join("a.txt"), "a\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "A"]);
    // The merge below advances br-a past A, so remember A itself.
    git(p, &["tag", "a-tip"]);

    git(p, &["checkout", "-q", "main"]);
    git(p, &["checkout", "-q", "-b", "br-b"]);
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "B"]);

    git(p, &["checkout", "-q", "br-a"]);
    git(p, &["merge", "-q", "--no-ff", "--no-edit", "br-b"]);
    git(p, &["checkout", "-q", "br-b"]);
    git(p, &["merge", "-q", "--no-ff", "--no-edit", "a-tip"]);

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("br-a...br-b")));

    // git rev-list --count br-a...br-b is 2: only the two cross merges. Hiding just one merge base
    // leaks the other into the count (3 commits).
    assert_eq!(
        total(&r).commits,
        2,
        "criss-cross merge bases must all be hidden:\n{r:#?}"
    );
}

/// Three commits on main with both tag flavors on the middle commit: the shape `git
/// describe`-driven release ranges produce. The tempdir is returned so the repository outlives the
/// test body.
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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    (dir, repo)
}

#[test]
fn lightweight_tag_range_is_unaffected() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (_dir, repo) = tag_fixture();
    let r = report(&repo, &options(Some("lightweight..HEAD")));

    assert_eq!(
        total(&r).commits,
        1,
        "lightweight..HEAD should count only the commit after the tag:\n{r:#?}"
    );
}

#[test]
fn annotated_tag_range_excludes_commits_behind_the_tag() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (_dir, repo) = tag_fixture();
    let r = report(&repo, &options(Some("annotated..HEAD")));

    // The tag object must be peeled to its commit before going into the hidden set; an unpeeled tag
    // OID matches nothing and the range silently degrades to whole-repo history (3 commits here).
    assert_eq!(
        total(&r).commits,
        1,
        "annotated..HEAD should count only the commit after the tag:\n{r:#?}"
    );
}

#[test]
fn annotated_tag_on_the_inclusion_side_resolves() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (_dir, repo) = tag_fixture();
    let r = report(&repo, &options(Some("HEAD~2..annotated")));

    assert_eq!(
        total(&r).commits,
        1,
        "HEAD~2..annotated should count only the tagged commit:\n{r:#?}"
    );
}

#[test]
fn a_bare_annotated_tag_resolves_to_its_target_commit() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let (dir, repo) = tag_fixture();
    let r = report(&repo, &options(Some("annotated")));

    // The tag points at the second of three commits, so its history is 2 deep.
    assert_eq!(
        total(&r).commits,
        2,
        "a bare annotated tag should walk from its target commit:\n{r:#?}"
    );

    // git peels tags recursively, so a tag pointing at another tag must resolve to the same commit.
    git(
        dir.path(),
        &["tag", "-a", "-m", "wrap", "nested", "annotated"],
    );
    let r = report(&repo, &options(Some("nested")));
    assert_eq!(
        total(&r).commits,
        2,
        "a tag-to-tag chain should peel to the same commit:\n{r:#?}"
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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    // Unlike the two-dot tests, `...` also feeds the endpoints to the merge-base computation, which
    // needs commit OIDs rather than tag OIDs.
    let r = report(&repo, &options(Some("main...feature-tag")));

    assert_eq!(
        total(&r).commits,
        2,
        "main...feature-tag should count the 2 divergent commits:\n{r:#?}"
    );
}

#[test]
fn filter_validation_precedes_the_walk() {
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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let mut opts = options(Some("no-such-ref..HEAD"));
    opts.since = Some("not-a-date".to_string());

    // Both the range and the date are bad. The date must win: filters are cheap to validate and
    // should error before a potentially expensive walk of the whole range even starts.
    let err = app::run(&repo, &opts).unwrap_err();
    assert!(
        matches!(err, git_stats::Error::InvalidDate { .. }),
        "expected InvalidDate before any range resolution, got: {err:?}"
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

    // One text file and one binary file (NUL bytes) in the same commit. `git log --numstat` lists
    // the binary file as `-  -  path` and `git diff --shortstat` reports "2 files changed, 2
    // insertions(+)".
    std::fs::write(p.join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(p.join("blob.bin"), [0u8, 159, 146, 150, 0, 10]).unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "add text and binary"]);

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("HEAD")));

    // The binary file counts as a changed file; only the text file contributes lines.
    assert_eq!(counts(total(&r)), (1, 2, 2, 0, 2), "{r:#?}");
}

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
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
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

/// `.mailmap` resolution must match `git shortlog -sne`: commits recorded under an old identity
/// fold into the mapped one, while authors the map does not mention stay untouched.
#[test]
fn mailmap_folds_identities_like_git_shortlog() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.name", "Old Name"]);
    git(p, &["config", "user.email", "old@example.com"]);

    std::fs::write(p.join("a.txt"), "a\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "as old identity"]);

    std::fs::write(
        p.join(".mailmap"),
        "New Name <new@example.com> <old@example.com>\n",
    )
    .unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "add mailmap"]);

    // Control: an author the mailmap does not mention.
    git(p, &["config", "user.name", "Other"]);
    git(p, &["config", "user.email", "other@example.com"]);
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "untouched author"]);

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let mut opts = options(Some("HEAD"));
    opts.email = true;
    let r = report(&repo, &opts);

    assert_eq!(
        author(&r, "New Name <new@example.com>").commits,
        2,
        "both old-identity commits should fold into the mapped one:\n{r:#?}"
    );
    let keys: Vec<&str> = r
        .stats
        .iter()
        .flat_map(|t| &t.rows)
        .map(|s| s.key.as_str())
        .collect();
    // The old identity must not leak into any row, and the unmapped author passes through.
    assert_eq!(
        keys,
        ["New Name <new@example.com>", "Other <other@example.com>"],
        "{r:#?}"
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
    let mut opts = options(Some("HEAD"));
    opts.email = true;
    opts.reviews = true;
    let r = report(&repo, &opts);
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
fn submodule_changes_count_like_git_numstat() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let inner = p.join("inner");
    std::fs::create_dir(&inner).unwrap();
    git(&inner, &["init", "-q", "-b", "main"]);
    git(&inner, &["config", "user.name", "Ada"]);
    git(&inner, &["config", "user.email", "ada@example.com"]);
    std::fs::write(inner.join("x.txt"), "x\n").unwrap();
    git(&inner, &["add", "."]);
    git(&inner, &["commit", "-q", "-m", "inner 1"]);
    let sha1 = git(&inner, &["rev-parse", "HEAD"]);
    std::fs::write(inner.join("y.txt"), "y\n").unwrap();
    git(&inner, &["add", "."]);
    git(&inner, &["commit", "-q", "-m", "inner 2"]);
    let sha2 = git(&inner, &["rev-parse", "HEAD"]);

    // Gitlink entries via plumbing keep the fixture hermetic: no .gitmodules, no clone, just a tree
    // entry of mode 160000 pointing at the inner commits.
    let outer = p.join("outer");
    std::fs::create_dir(&outer).unwrap();
    git(&outer, &["init", "-q", "-b", "main"]);
    git(&outer, &["config", "user.name", "Ada"]);
    git(&outer, &["config", "user.email", "ada@example.com"]);
    std::fs::write(outer.join("r.txt"), "readme\n").unwrap();
    git(&outer, &["add", "."]);
    git(&outer, &["commit", "-q", "-m", "base"]);
    let link = format!("160000,{sha1},sub");
    git(&outer, &["update-index", "--add", "--cacheinfo", &link]);
    git(&outer, &["commit", "-q", "-m", "add sub"]);
    let link = format!("160000,{sha2},sub");
    git(&outer, &["update-index", "--add", "--cacheinfo", &link]);
    git(&outer, &["commit", "-q", "-m", "bump sub"]);

    let repo = Repo::open(&outer, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("HEAD")));

    // git log --numstat: base is `1 0 r.txt`, adding the gitlink is `1 0 sub` (one "Subproject
    // commit" line), repointing it is `1 1 sub`. Totals: 3 files, +3, -1.
    assert_eq!(counts(total(&r)), (3, 3, 3, 1, 2), "{r:#?}");
}

/// A renamed submodule is paired by the rename tracker into git's single `0 0 old => new` entry, so
/// it counts as one changed file with no line changes rather than as an addition plus a deletion.
#[test]
fn gitlink_renames_count_as_one_unchanged_file() {
    if !git_available() {
        eprintln!("git not available; skipping integration test");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let inner = p.join("inner");
    std::fs::create_dir(&inner).unwrap();
    git(&inner, &["init", "-q", "-b", "main"]);
    git(&inner, &["config", "user.name", "Ada"]);
    git(&inner, &["config", "user.email", "ada@example.com"]);
    std::fs::write(inner.join("x.txt"), "x\n").unwrap();
    git(&inner, &["add", "."]);
    git(&inner, &["commit", "-q", "-m", "inner"]);
    let sha = git(&inner, &["rev-parse", "HEAD"]);

    let outer = p.join("outer");
    std::fs::create_dir(&outer).unwrap();
    git(&outer, &["init", "-q", "-b", "main"]);
    git(&outer, &["config", "user.name", "Ada"]);
    git(&outer, &["config", "user.email", "ada@example.com"]);
    // The walk reads its diff options from repository config, so pin rename detection rather than
    // inheriting whatever the developer set globally.
    git(&outer, &["config", "diff.renames", "true"]);
    std::fs::write(outer.join("r.txt"), "readme\n").unwrap();
    git(&outer, &["add", "."]);
    git(&outer, &["commit", "-q", "-m", "base"]);
    let link = format!("160000,{sha},sub");
    git(&outer, &["update-index", "--add", "--cacheinfo", &link]);
    git(&outer, &["commit", "-q", "-m", "add sub"]);
    git(&outer, &["update-index", "--force-remove", "sub"]);
    let link = format!("160000,{sha},newsub");
    git(&outer, &["update-index", "--add", "--cacheinfo", &link]);
    git(&outer, &["commit", "-q", "-m", "rename sub"]);

    let repo = Repo::open(&outer, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("HEAD")));

    // base is 1 file +1, adding the gitlink is 1 file +1, and the rename is 1 file whose
    // `Subproject commit` line is unchanged.
    assert_eq!(counts(total(&r)), (3, 3, 2, 0, 2), "{r:#?}");
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
    // `reads_a_real_repository`.
    assert!(repo.is_shallow(), "depth-2 clone should detect as shallow");
    let r = report(&repo, &options(Some("HEAD")));

    // git log --numstat here shows c3 as +1 and the boundary commit c2 as its whole tree (+2),
    // exactly like a root commit, so the total is +3.
    assert_eq!(
        total(&r).commits,
        2,
        "both retained commits should count:\n{r:#?}"
    );
    assert_eq!(
        total(&r).insertions,
        3,
        "the boundary commit should diff against the empty tree:\n{r:#?}"
    );
}

#[test]
fn shallow_boundary_merges_diff_like_root_commits() {
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

    // History whose second-newest commit is a merge: base, a feature branch, a diverging main, the
    // merge, and one commit on top.
    std::fs::write(origin.join("a.txt"), "a\n").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-q", "-m", "base"]);
    git(&origin, &["checkout", "-q", "-b", "feat"]);
    std::fs::write(origin.join("b.txt"), "b\n").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-q", "-m", "feat"]);
    git(&origin, &["checkout", "-q", "main"]);
    std::fs::write(origin.join("c.txt"), "c\n").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-q", "-m", "main side"]);
    git(&origin, &["merge", "-q", "--no-ff", "--no-edit", "feat"]);
    std::fs::write(origin.join("d.txt"), "d\n").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-q", "-m", "top"]);

    // The depth-2 clone keeps "top" and the merge, grafting the merge as the parentless shallow
    // boundary. Its header still names two parents, but git neither treats it as a merge nor skips
    // it: git log --numstat shows its entire tree (a, b, c), plus d from "top", so 4 files and +4
    // lines.
    let url = format!("file://{}", origin.display());
    git(p, &["clone", "-q", "--depth", "2", &url, "shallow"]);

    let repo = Repo::open(p.join("shallow"), Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("HEAD")));

    assert_eq!(
        total(&r).commits,
        2,
        "both retained commits should count:\n{r:#?}"
    );
    assert_eq!(
        total(&r).num_files,
        4,
        "the grafted merge should contribute its whole tree:\n{r:#?}"
    );
    assert_eq!(
        total(&r).insertions,
        4,
        "expected +4 total insertions:\n{r:#?}"
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
    let err = app::run(&repo, &options(Some("HEAD"))).unwrap_err();
    assert!(
        err.to_string().contains(&bad),
        "error should name the undecodable commit {bad}: {err}"
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

    let repo = Repo::open(p, Vcs::Git).unwrap();
    let r = report(&repo, &options(Some("HEAD")));

    // Four commits (base, feature, main, merge); the merge contributes no lines, so insertions
    // total only the three single-line file additions.
    assert_eq!(
        total(&r).commits,
        4,
        "merge commit should be counted:\n{r:#?}"
    );
    assert_eq!(
        total(&r).insertions,
        3,
        "merge should contribute no insertions:\n{r:#?}"
    );
}
