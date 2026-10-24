//! Integration tests against real, throwaway jj workspaces. They cover what the git tests cannot:
//! workspace detection, revset evaluation through the `jj` CLI, and reading jj's git store.

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use git_stats::model::Vcs;
use git_stats::repo::Repo;

use common::{counts, git, has_author, options, report, run, total};

/// A temporary directory holding a hermetic jj config next to a workspace.
struct Fixture {
    _dir: tempfile::TempDir,
    config: PathBuf,
    ws: PathBuf,
    // jj 0.38 and newer keep repo config under the user config directory, keyed by repo path,
    // outside the temporary directory. `Drop` removes it.
    repo_config_dir: Option<PathBuf>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(dir) = &self.repo_config_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl Fixture {
    /// Create a workspace with `jj git init` and the given init flags.
    fn init(flags: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        std::fs::write(
            &config,
            "[user]\nname = \"Ada\"\nemail = \"ada@example.com\"\n",
        )
        .unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir(&ws).unwrap();
        let mut fixture = Self {
            _dir: dir,
            config,
            ws,
            repo_config_dir: None,
        };
        let mut args = vec!["git", "init"];
        args.extend_from_slice(flags);
        fixture.jj(&args);
        let repo_config = PathBuf::from(fixture.jj(&["config", "path", "--repo"]));
        fixture.repo_config_dir = repo_config.parent().map(Path::to_path_buf);
        // git-stats runs jj with the host's user config, which may lack an identity, name another
        // one, or sign commits with the host's key. Repo config applies to those runs too, so their
        // snapshots keep a fixed author and stay unsigned. Color is forced on to prove that
        // git-stats turns it off.
        fixture.jj(&["config", "set", "--repo", "user.name", "Ada"]);
        fixture.jj(&["config", "set", "--repo", "user.email", "ada@example.com"]);
        fixture.jj(&["config", "set", "--repo", "signing.behavior", "drop"]);
        fixture.jj(&["config", "set", "--repo", "ui.color", "always"]);
        fixture
    }

    /// Run jj in the workspace with only the fixture's config, and return its trimmed stdout.
    fn jj(&self, args: &[&str]) -> String {
        let out = Command::new("jj")
            .current_dir(&self.ws)
            // The repo config forces color on for git-stats' own jj runs.
            .arg("--color=never")
            .args(args)
            .env("JJ_CONFIG", &self.config)
            .output()
            .expect("jj should be installed");
        assert!(
            out.status.success(),
            "jj {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn write(&self, file: &str, contents: &str) {
        std::fs::write(self.ws.join(file), contents).unwrap();
    }
}

/// The Total row's columns after the label, read from the binary's plain text: commits, files,
/// insertions, deletions, net.
fn text_totals(out: &str) -> Vec<&str> {
    out.lines()
        .find(|l| l.starts_with("Total"))
        .unwrap_or_else(|| panic!("no Total row in:\n{out}"))
        .split_whitespace()
        .skip(1)
        .collect()
}

/// Whether a usable `jj` binary is on PATH; tests skip gracefully without one.
fn jj_available() -> bool {
    Command::new("jj").arg("--version").output().is_ok()
}

/// Two commits, then an edit left in the working-copy commit `@`. jj has not snapshotted the edit
/// yet, so only git-stats' own `jj log` can pick it up.
fn two_commits_and_a_pending_edit(f: &Fixture) {
    f.write("a.txt", "1\n2\n");
    f.jj(&["commit", "-m", "first"]);
    f.write("a.txt", "1\n2\n3\n");
    f.jj(&["commit", "-m", "second"]);
    f.write("a.txt", "1\n2\n3\n4\n");
}

#[test]
fn default_revset_counts_the_working_copy_and_skips_root() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    two_commits_and_a_pending_edit(&f);

    let repo = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&repo, &options(None));

    // `::@` is first, second, and the snapshotted @, each adding one file's lines: +2, +1, +1. jj's
    // virtual root commit has no git object, so reading it would fail the whole run.
    assert_eq!(counts(total(&r)), (3, 3, 4, 0, 4), "{r:#?}");
}

#[test]
fn revsets_resolve_change_ids() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    two_commits_and_a_pending_edit(&f);
    let first = f.jj(&[
        "log",
        "--no-graph",
        "-r",
        "description(exact:\"first\\n\")",
        "-T",
        "change_id",
    ]);

    let repo = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&repo, &options(Some(&format!("{first}..@-"))));

    // jj's `x..y` is `::y ~ ::x`, which leaves only "second".
    assert_eq!(counts(total(&r)), (1, 1, 1, 0, 1), "{r:#?}");
}

#[test]
fn colocated_workspace_defaults_to_jj_unless_git_is_forced() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--colocate"]);
    two_commits_and_a_pending_edit(&f);

    // Forced git reads HEAD, which jj keeps at @-, so the pending edit is out.
    let git = Repo::open(&f.ws, Vcs::Git).unwrap();
    let git_r = report(&git, &options(None));
    assert_eq!(counts(total(&git_r)), (2, 2, 3, 0, 3), "git: {git_r:#?}");

    let jj = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&jj, &options(None));
    assert_eq!(counts(total(&r)), (3, 3, 4, 0, 4), "jj: {r:#?}");
    // The same commits read through either backend give the same report.
    assert_eq!(report(&jj, &options(Some("::@-"))), git_r);

    // In jj mode the range is a revset, where `HEAD` names nothing.
    assert!(run(&jj, &options(Some("HEAD"))).is_err());
}

#[test]
fn non_colocated_workspace_applies_the_workspace_mailmap() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    f.write(".mailmap", "Ada Lovelace <ada@example.com>\n");
    f.jj(&["commit", "-m", "add mailmap"]);

    let repo = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&repo, &options(Some("@-")));

    // jj's git store is bare and lives inside `.jj`, so the mailmap must come from the workspace
    // rather than a git worktree.
    assert!(has_author(&r, "Ada Lovelace"), "{r:#?}");
}

/// A non-colocated store has no index and an unborn `HEAD`, so attributes must come from the
/// workspace's own `.gitattributes`, as git takes them from a worktree.
#[test]
fn workspace_gitattributes_apply_without_a_git_worktree() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    f.write(".gitattributes", "*.bin -diff\n");
    f.write("a.bin", "1\n2\n");
    f.jj(&["commit", "-m", "first"]);
    f.write("a.bin", "1\n2\n3\n");
    f.jj(&["commit", "-m", "second"]);

    let repo = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&repo, &options(Some("::@-")));

    // `-diff` makes `a.bin` binary: each change counts as a file with no lines, so only the one
    // `.gitattributes` line is an insertion.
    assert_eq!(counts(total(&r)), (2, 3, 1, 0, 1), "{r:#?}");
}

#[test]
fn secondary_workspace_mailmap_wins_over_the_git_worktree() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    // In a colocated repository, the git worktree is the main workspace.
    let f = Fixture::init(&["--colocate"]);
    f.write("a.txt", "1\n");
    f.jj(&["commit", "-m", "first"]);
    f.write(".mailmap", "Main Mapping <ada@example.com>\n");
    let second = f.ws.parent().unwrap().join("second");
    f.jj(&["workspace", "add", second.to_str().unwrap()]);
    std::fs::write(
        second.join(".mailmap"),
        "Second Mapping <ada@example.com>\n",
    )
    .unwrap();

    let repo = Repo::open(&second, Vcs::Auto).unwrap();
    let r = report(&repo, &options(Some("::@")));

    // Both files map the same email. The workspace being read decides, as the worktree does for
    // git.
    assert!(has_author(&r, "Second Mapping"), "{r:#?}");
    assert!(!has_author(&r, "Main Mapping"), "{r:#?}");
}

#[test]
fn git_repository_nested_in_a_jj_workspace_stays_git() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    two_commits_and_a_pending_edit(&f);
    let nested = f.ws.join("nested");
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "main"]);
    std::fs::write(nested.join("b.txt"), "x\n").unwrap();
    git(&nested, &["add", "."]);
    git(
        &nested,
        &["-c", "user.name=Bob", "-c", "user.email=bob@example.com"]
            .into_iter()
            .chain(["commit", "-q", "-m", "nested"])
            .collect::<Vec<_>>(),
    );

    let repo = Repo::open(&nested, Vcs::Auto).unwrap();
    let r = report(&repo, &options(None));

    // The nearer `.git` wins: only the nested repository's one commit counts.
    assert_eq!(counts(total(&r)), (1, 1, 1, 0, 1), "{r:#?}");
    assert!(has_author(&r, "Bob"), "{r:#?}");
}

/// A conflicted commit "right" rebased onto "left", and `@` on top of it resolving `a.txt` to two
/// lines.
fn conflict_fixture() -> Fixture {
    let f = Fixture::init(&["--no-colocate"]);
    f.write("a.txt", "1\n2\n3\n4\n5\n");
    f.jj(&["commit", "-m", "base"]);
    f.write("a.txt", "left\n");
    f.jj(&["commit", "-m", "left"]);
    f.jj(&["new", "subject(\"base\")"]);
    f.write("a.txt", "right\n");
    f.jj(&["commit", "-m", "right"]);
    f.jj(&[
        "rebase",
        "-r",
        "subject(\"right\")",
        "-d",
        "subject(\"left\")",
    ]);
    f.jj(&["new", "conflicts()"]);
    f.write("a.txt", "resolved\nextra\n");
    f
}

#[test]
fn conflicted_commits_count_their_materialized_diff() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = conflict_fixture();
    let repo = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&repo, &options(Some("conflicts()::@")));

    // The conflicted commit replaces one line with conflict markers, and the resolution replaces
    // the markers with two lines, so each touches only a.txt and the pair adds one line more than
    // it removes, whatever the marker style. A git diff would also count the `.jjconflict-*` files
    // that jj stores in a conflicted commit's tree.
    let t = total(&r);
    assert_eq!((t.commits, t.num_files, t.net), (2, 2, 1), "{r:#?}");
}

/// jj counts a conflicted merge against its auto-merged parents, so a merge that adds a file gets
/// counts from jj. A merge adds nothing in git's numstat, so git-stats must drop them.
#[test]
fn conflicted_merges_add_nothing() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    f.write("a.txt", "base\n");
    f.jj(&["commit", "-m", "base"]);
    f.write("a.txt", "left\n");
    f.jj(&["commit", "-m", "left"]);
    f.jj(&["new", "subject(\"base\")"]);
    f.write("a.txt", "right\n");
    f.jj(&["commit", "-m", "right"]);
    f.jj(&["new", "subject(\"left\")", "subject(\"right\")"]);
    f.write("b.txt", "b\n");
    let repo = Repo::open(&f.ws, Vcs::Auto).unwrap();
    let r = report(&repo, &options(Some("@ & conflicts()")));

    assert_eq!(counts(total(&r)), (1, 0, 0, 0, 0), "{r:#?}");
}

/// jj wraps log lines at `COLUMNS` when `ui.log-word-wrap` is set, even with stdout piped. A
/// wrapped line would split a conflicted commit's counts from its ID.
#[test]
fn word_wrap_config_does_not_split_log_lines() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = conflict_fixture();
    f.jj(&["config", "set", "--repo", "ui.log-word-wrap", "true"]);

    let out = Command::new(env!("CARGO_BIN_EXE_git-stats"))
        .current_dir(&f.ws)
        .arg("conflicts()::@")
        // Narrower than a commit ID plus its three counts.
        .env("COLUMNS", "20")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text_totals(&stdout)[..2], ["2", "2"], "out:\n{stdout}");
}

#[test]
fn forced_jj_ignores_a_nearer_git_repository() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    two_commits_and_a_pending_edit(&f);
    let nested = f.ws.join("nested");
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "main"]);

    let repo = Repo::open(&nested, Vcs::Jj).unwrap();
    // `::@-` leaves out whatever the snapshot of `@` picks up under `nested`.
    let r = report(&repo, &options(Some("::@-")));

    assert_eq!(counts(total(&r)), (2, 2, 3, 0, 3), "{r:#?}");
}

#[test]
fn forced_jj_outside_a_workspace_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = Repo::open(dir.path(), Vcs::Jj);
    assert!(
        matches!(result, Err(git_stats::Error::NoJjWorkspace { .. })),
        "expected NoJjWorkspace"
    );
}

/// `main` opens the repository at `.`, so the search for `.jj` must climb from the current
/// directory, not only from an absolute path.
#[test]
fn binary_finds_the_workspace_from_a_subdirectory() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--no-colocate"]);
    two_commits_and_a_pending_edit(&f);
    let sub = f.ws.join("sub");
    std::fs::create_dir(&sub).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_git-stats"))
        .current_dir(&sub)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        text_totals(&stdout),
        ["3", "3", "+4", "0", "+4"],
        "out:\n{stdout}"
    );
}

/// git sets `GIT_DIR` when it runs hooks. In a colocated workspace, a hook that runs `git stats`
/// must then read git's `HEAD`, not jj's `::@`.
#[test]
fn git_dir_in_a_colocated_workspace_selects_git() {
    if !jj_available() {
        eprintln!("jj not available; skipping integration test");
        return;
    }
    let f = Fixture::init(&["--colocate"]);
    two_commits_and_a_pending_edit(&f);

    let out = Command::new(env!("CARGO_BIN_EXE_git-stats"))
        .current_dir(&f.ws)
        .env("GIT_DIR", f.ws.join(".git"))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();

    // `HEAD` is @-, so only the two committed changes count.
    assert_eq!(
        text_totals(&stdout),
        ["2", "2", "+3", "0", "+3"],
        "out:\n{stdout}"
    );
}
