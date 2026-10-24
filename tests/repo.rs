//! Repository tests: revision ranges, Numstat, and `.mailmap`, over repositories that
//! [`RepoBuilder`] writes.

mod common;

use git_stats::repo::Repo;
use gix::ObjectId;
use tempfile::TempDir;

use common::{Commit, RepoBuilder, author, counts, options, report, reviewer, total};

fn ada() -> Commit {
    Commit::default().author("Ada", "ada@example.com")
}

/// The number of commits `range` covers.
fn commits_in(repo: &Repo, range: &str) -> u64 {
    total(&report(repo, &options(Some(range)))).commits
}

#[test]
fn counts_lines_and_review_trailers() {
    let mut b = RepoBuilder::new();
    let first = b.commit(
        ada()
            .message("first\n\nReviewed-by: Rev Iewer <rev@example.com>\n")
            .file("a.txt", "line1\nline2\n"),
    );
    b.commit(ada().parent(first).file("a.txt", "line1\nline2\nline3\n"));
    let (_dir, repo) = b.open();

    // Control for the shallow-clone probe: a full repository must not warn.
    assert!(!repo.is_shallow(), "full repository detected as shallow");
    let mut opts = options(Some("HEAD"));
    opts.reviews = true;
    let r = report(&repo, &opts);

    // Ada: 2 commits; root adds 2 lines, the follow-up adds 1, so +3 insertions.
    assert_eq!(author(&r, "Ada").insertions, 3, "{r:#?}");
    assert_eq!(reviewer(&r, "Rev Iewer").commits, 1, "{r:#?}");
}

/// A base commit and the branches `main` and `feature`, one commit each, growing from it.
struct Diverged {
    b: RepoBuilder,
    main: ObjectId,
    feature: ObjectId,
}

fn diverged() -> Diverged {
    let mut b = RepoBuilder::new();
    let base = b.commit(ada().file("a.txt", "a\n"));
    let main = b.commit(ada().parent(base).file("a.txt", "a\n").file("m.txt", "m\n"));
    let feature = b.commit(
        ada()
            .parent(base)
            .file("a.txt", "a\n")
            .file("f0.txt", "f\n"),
    );
    b.branch("main", main);
    b.branch("feature", feature);
    Diverged { b, main, feature }
}

#[test]
fn range_excludes_commits_on_the_excluded_side() {
    let mut d = diverged();
    let feature = d.b.commit(
        ada()
            .parent(d.feature)
            .file("a.txt", "a\n")
            .file("f0.txt", "f\n")
            .file("f1.txt", "f\n"),
    );
    d.b.branch("feature", feature);
    let (_dir, repo) = d.b.open();
    // `main..feature` excludes main's commits and the base, leaving the 2 feature commits.
    assert_eq!(commits_in(&repo, "main..feature"), 2);
}

#[test]
fn symmetric_difference_range_excludes_the_common_ancestor() {
    let (_dir, repo) = diverged().b.open();
    // The symmetric difference drops the shared base, leaving the 2 divergent commits.
    assert_eq!(commits_in(&repo, "main...feature"), 2);
}

#[test]
fn symmetric_difference_hides_every_merge_base() {
    // Criss-cross history: each branch merges the other's first commit, so br-a...br-b has two
    // merge bases (A and B), not one.
    let mut b = RepoBuilder::new();
    let base = b.commit(ada().file("base.txt", "base\n"));
    let a = b.commit(
        ada()
            .parent(base)
            .file("base.txt", "base\n")
            .file("a.txt", "a\n"),
    );
    let bb = b.commit(
        ada()
            .parent(base)
            .file("base.txt", "base\n")
            .file("b.txt", "b\n"),
    );
    let both = |c: Commit| {
        c.file("base.txt", "base\n")
            .file("a.txt", "a\n")
            .file("b.txt", "b\n")
    };
    let br_a = b.commit(both(ada().parent(a).parent(bb)));
    let br_b = b.commit(both(ada().parent(bb).parent(a)));
    b.branch("br-a", br_a);
    b.branch("br-b", br_b);
    let (_dir, repo) = b.open();

    // git rev-list --count br-a...br-b is 2: only the two cross merges. Hiding just one merge base
    // leaks the other into the count (3 commits).
    assert_eq!(commits_in(&repo, "br-a...br-b"), 2);
}

/// Three commits with both tag flavors on the middle one: the shape `git describe`-driven release
/// ranges produce. Returns the builder and the annotated tag object.
fn tagged() -> (RepoBuilder, ObjectId) {
    let mut b = RepoBuilder::new();
    let c1 = b.commit(ada().file("a.txt", "x\n"));
    let c2 = b.commit(ada().parent(c1).file("a.txt", "x\n").file("b.txt", "x\n"));
    b.commit(
        ada()
            .parent(c2)
            .file("a.txt", "x\n")
            .file("b.txt", "x\n")
            .file("c.txt", "x\n"),
    );
    let annotated = b.annotated_tag("annotated", c2);
    b.lightweight_tag("lightweight", c2);
    (b, annotated)
}

#[test]
fn lightweight_tag_range_is_unaffected() {
    let (_dir, repo) = tagged().0.open();
    assert_eq!(commits_in(&repo, "lightweight..HEAD"), 1);
}

#[test]
fn annotated_tag_range_excludes_commits_behind_the_tag() {
    let (_dir, repo) = tagged().0.open();
    // The tag object must be peeled to its commit before going into the hidden set; an unpeeled tag
    // OID matches nothing and the range silently degrades to whole-repo history (3 commits here).
    assert_eq!(commits_in(&repo, "annotated..HEAD"), 1);
}

#[test]
fn annotated_tag_on_the_inclusion_side_resolves() {
    let (_dir, repo) = tagged().0.open();
    assert_eq!(commits_in(&repo, "HEAD~2..annotated"), 1);
}

#[test]
fn a_bare_annotated_tag_resolves_to_its_target_commit() {
    let (mut b, annotated) = tagged();
    // git peels tags recursively, so a tag pointing at another tag must resolve to the same commit.
    b.annotated_tag("nested", annotated);
    let (_dir, repo) = b.open();

    // The tag points at the second of three commits, so its history is 2 deep.
    assert_eq!(commits_in(&repo, "annotated"), 2);
    assert_eq!(commits_in(&repo, "nested"), 2);
}

#[test]
fn symmetric_difference_with_an_annotated_tag_resolves() {
    let mut d = diverged();
    d.b.annotated_tag("feature-tag", d.feature);
    let (_dir, repo) = d.b.open();
    // Unlike the two-dot tests, `...` also feeds the endpoints to the merge-base computation, which
    // needs commit OIDs rather than tag OIDs.
    assert_eq!(commits_in(&repo, "main...feature-tag"), 2);
}

#[test]
fn merge_commits_are_counted_but_add_no_lines() {
    let mut d = diverged();
    d.b.commit(
        ada()
            .parent(d.main)
            .parent(d.feature)
            .file("a.txt", "a\n")
            .file("m.txt", "m\n")
            .file("f0.txt", "f\n"),
    );
    let (_dir, repo) = d.b.open();
    let r = report(&repo, &options(Some("HEAD")));

    // Four commits (base, main, feature, merge); the merge contributes no lines, so insertions
    // total only the three single-line file additions.
    assert_eq!(counts(total(&r)), (4, 3, 3, 0, 3), "{r:#?}");
}

#[test]
fn shallow_boundary_merges_diff_like_root_commits() {
    // The merge is the shallow boundary, grafted as parentless. Its header still names two
    // parents, but git neither treats it as a merge nor skips it: git log --numstat shows its
    // entire tree (a, m, f0), plus t from "top", so 4 files and +4 lines.
    let mut d = diverged();
    let merge = d.b.commit(
        ada()
            .parent(d.main)
            .parent(d.feature)
            .file("a.txt", "a\n")
            .file("m.txt", "m\n")
            .file("f0.txt", "f\n"),
    );
    d.b.commit(
        ada()
            .parent(merge)
            .file("a.txt", "a\n")
            .file("m.txt", "m\n")
            .file("f0.txt", "f\n")
            .file("t.txt", "t\n"),
    );
    d.b.shallow(&[merge]);
    let (_dir, repo) = d.b.open();
    assert!(repo.is_shallow());

    let r = report(&repo, &options(Some("HEAD")));
    assert_eq!(counts(total(&r)), (2, 4, 4, 0, 4), "{r:#?}");
}

#[test]
fn binary_file_changes_count_as_changed_files() {
    // One text file and one binary file (NUL bytes) in the same commit. `git log --numstat` lists
    // the binary file as `-  -  path` and `git diff --shortstat` reports "2 files changed, 2
    // insertions(+)".
    let mut b = RepoBuilder::new();
    b.commit(
        ada()
            .file("a.txt", "one\ntwo\n")
            .file("blob.bin", [0u8, 159, 146, 150, 0, 10]),
    );
    let (_dir, repo) = b.open();
    let r = report(&repo, &options(Some("HEAD")));

    // The binary file counts as a changed file; only the text file contributes lines.
    assert_eq!(counts(total(&r)), (1, 2, 2, 0, 2), "{r:#?}");
}

/// Two arbitrary commit IDs for submodule entries to point at. Gitlinks name commits in another
/// repository, so these need not exist here.
fn submodule_commits() -> (ObjectId, ObjectId) {
    (
        ObjectId::from_hex(b"1111111111111111111111111111111111111111").unwrap(),
        ObjectId::from_hex(b"2222222222222222222222222222222222222222").unwrap(),
    )
}

#[test]
fn submodule_changes_count_like_git_numstat() {
    let (sha1, sha2) = submodule_commits();
    let mut b = RepoBuilder::new();
    let readme = || ada().file("r.txt", "readme\n");
    let base = b.commit(readme());
    let added = b.commit(readme().parent(base).gitlink("sub", sha1));
    b.commit(readme().parent(added).gitlink("sub", sha2));
    let (_dir, repo) = b.open();
    let r = report(&repo, &options(Some("HEAD")));

    // git log --numstat: base is `1 0 r.txt`, adding the gitlink is `1 0 sub` (one "Subproject
    // commit" line), repointing it is `1 1 sub`. Totals: 3 files, +3, -1.
    assert_eq!(counts(total(&r)), (3, 3, 3, 1, 2), "{r:#?}");
}

/// A renamed submodule is paired by the rename tracker into git's single `0 0 old => new` entry, so
/// it counts as one changed file with no line changes rather than as an addition plus a deletion.
#[test]
fn gitlink_renames_count_as_one_unchanged_file() {
    let (sha, _) = submodule_commits();
    let mut b = RepoBuilder::new();
    // The walk reads its diff options from repository config, so pin rename detection.
    b.config("diff.renames", "true");
    let readme = || ada().file("r.txt", "readme\n");
    let base = b.commit(readme());
    let added = b.commit(readme().parent(base).gitlink("sub", sha));
    b.commit(readme().parent(added).gitlink("newsub", sha));
    let (_dir, repo) = b.open();
    let r = report(&repo, &options(Some("HEAD")));

    // base is 1 file +1, adding the gitlink is 1 file +1, and the rename is 1 file whose
    // `Subproject commit` line is unchanged.
    assert_eq!(counts(total(&r)), (3, 3, 2, 0, 2), "{r:#?}");
}

/// A `.mailmap` with an old identity, two commits under it, and one by an Author it does not
/// mention. One commit names the old identity as its Reviewer.
fn mailmapped() -> (TempDir, Repo) {
    let mut b = RepoBuilder::new();
    b.mailmap("New Name <new@example.com> <old@example.com>\n");
    let old = |c: Commit| c.author("Old Name", "old@example.com");
    let c1 = b.commit(old(Commit::default()).file("a.txt", "a\n"));
    let c2 = b.commit(
        old(Commit::default())
            .parent(c1)
            .message("change\n\nReviewed-by: Old Name <old@example.com>\n")
            .file("a.txt", "a\nb\n"),
    );
    b.commit(
        Commit::default()
            .author("Other", "other@example.com")
            .parent(c2)
            .file("a.txt", "a\nb\n")
            .file("b.txt", "b\n"),
    );
    b.open()
}

/// `.mailmap` resolution must match `git shortlog -sne`: commits recorded under an old identity
/// fold into the mapped one, while Authors the map does not mention stay untouched.
#[test]
fn mailmap_folds_author_identities() {
    let (_dir, repo) = mailmapped();
    let mut opts = options(Some("HEAD"));
    opts.email = true;
    let r = report(&repo, &opts);

    let keys: Vec<&str> = r
        .stats
        .iter()
        .flat_map(|t| &t.rows)
        .map(|s| s.key.as_str())
        .collect();
    // The old identity must not leak into any row, and the unmapped Author passes through.
    assert_eq!(
        keys,
        ["New Name <new@example.com>", "Other <other@example.com>"],
        "{r:#?}"
    );
    assert_eq!(
        author(&r, "New Name <new@example.com>").commits,
        2,
        "{r:#?}"
    );
}

#[test]
fn mailmap_folds_reviewer_identities() {
    let (_dir, repo) = mailmapped();
    let mut opts = options(Some("HEAD"));
    opts.email = true;
    opts.reviews = true;
    let r = report(&repo, &opts);

    assert_eq!(r.reviews.len(), 1, "{r:#?}");
    assert_eq!(
        reviewer(&r, "New Name <new@example.com>").commits,
        1,
        "{r:#?}"
    );
}
