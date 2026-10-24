//! Repository tests: revision ranges, Numstat, Reviewers, and `.mailmap`, through [`Repo::read`]
//! over repositories that [`RepoBuilder`] writes.

mod common;

use git_stats::model::{Contributor, DiffStat};
use git_stats::repo::Repo;
use gix::ObjectId;
use tempfile::TempDir;

use common::{Commit, RepoBuilder, read, sum};

fn ada() -> Commit {
    Commit::default().author("Ada", "ada@example.com")
}

fn who(name: &str, email: &str) -> Contributor {
    Contributor {
        name: name.to_string(),
        email: email.to_string(),
    }
}

/// The number of commits `range` covers.
fn commits_in(repo: &Repo, range: &str) -> usize {
    read(repo, Some(range)).len()
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
    let commits = repo.read(Some("HEAD"), |_| true, true).unwrap();

    // The root adds 2 lines and the follow-up 1. Reviewers come one list per commit, newest first.
    let want = DiffStat {
        insertions: 3,
        deletions: 0,
        files: 2,
    };
    assert_eq!(sum(&commits.kept), (2, want), "{commits:#?}");
    let rev = who("Rev Iewer", "rev@example.com");
    assert_eq!(commits.reviewers, Some(vec![vec![], vec![rev]]));
    // Not asked for, Reviewers are absent rather than empty.
    let unread = repo.read(Some("HEAD"), |_| true, false).unwrap();
    assert_eq!(unread.reviewers, None);
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
    let commits = read(&repo, Some("HEAD"));

    // Four commits (base, main, feature, merge); the merge contributes no lines, so insertions
    // total only the three single-line file additions.
    let want = DiffStat {
        insertions: 3,
        deletions: 0,
        files: 3,
    };
    assert_eq!(sum(&commits), (4, want), "{commits:#?}");
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

    let commits = read(&repo, Some("HEAD"));
    let want = DiffStat {
        insertions: 4,
        deletions: 0,
        files: 4,
    };
    assert_eq!(sum(&commits), (2, want), "{commits:#?}");
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
    let commits = read(&repo, Some("HEAD"));

    // The binary file counts as a changed file; only the text file contributes lines.
    let want = DiffStat {
        insertions: 2,
        deletions: 0,
        files: 2,
    };
    assert_eq!(sum(&commits), (1, want), "{commits:#?}");
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
    let commits = read(&repo, Some("HEAD"));

    // git log --numstat: base is `1 0 r.txt`, adding the gitlink is `1 0 sub` (one "Subproject
    // commit" line), repointing it is `1 1 sub`. Totals: 3 files, +3, -1.
    let want = DiffStat {
        insertions: 3,
        deletions: 1,
        files: 3,
    };
    assert_eq!(sum(&commits), (3, want), "{commits:#?}");
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
    let commits = read(&repo, Some("HEAD"));

    // base is 1 file +1, adding the gitlink is 1 file +1, and the rename is 1 file whose
    // `Subproject commit` line is unchanged.
    let want = DiffStat {
        insertions: 2,
        deletions: 0,
        files: 3,
    };
    assert_eq!(sum(&commits), (3, want), "{commits:#?}");
}

/// git diffs a file swapped with a submodule as one file: the file's lines on its side, and the
/// one `Subproject commit` line on the other. A binary file on either side makes the whole entry
/// binary, which git shows as `-  -  x`.
#[test]
fn file_and_submodule_swaps_count_like_git_numstat() {
    let (sha, _) = submodule_commits();
    let binary = [0u8, 159, 146, 150, 0, 10];
    let mut b = RepoBuilder::new();
    let c1 = b.commit(ada().file("x", "1\n2\n3\n"));
    let c2 = b.commit(ada().parent(c1).gitlink("x", sha));
    let c3 = b.commit(ada().parent(c2).file("x", "a\nb\n"));
    let c4 = b.commit(ada().parent(c3).gitlink("x", sha));
    let c5 = b.commit(ada().parent(c4).file("x", binary));
    b.commit(ada().parent(c5).gitlink("x", sha));
    let (_dir, repo) = b.open();
    let diffs: Vec<DiffStat> = read(&repo, Some("HEAD"))
        .into_iter()
        .map(|(_, diff)| diff)
        .collect();

    // Newest first, as `git log --numstat` prints them.
    let one_file = |insertions, deletions| DiffStat {
        insertions,
        deletions,
        files: 1,
    };
    let want = [
        one_file(0, 0),
        one_file(0, 0),
        one_file(1, 2),
        one_file(2, 1),
        one_file(1, 3),
        one_file(3, 0),
    ];
    assert_eq!(diffs, want);
}

/// A directory is not a file. When a file replaces a directory, or the reverse, git counts the
/// file and each file in the directory, but not the directory itself.
#[test]
fn directories_swapped_with_files_count_only_the_files() {
    let mut b = RepoBuilder::new();
    let c1 = b.commit(ada().file("dir/x", "1\n2\n"));
    let c2 = b.commit(ada().parent(c1).file("dir", "a\n"));
    b.commit(ada().parent(c2).file("dir/y", "1\n2\n3\n"));
    let (_dir, repo) = b.open();
    let diffs: Vec<DiffStat> = read(&repo, Some("HEAD"))
        .into_iter()
        .map(|(_, diff)| diff)
        .collect();

    // git log --numstat, newest first: `0 1 dir` and `3 0 dir/y`; `1 0 dir` and `0 2 dir/x`;
    // `2 0 dir/x`.
    let want = [
        DiffStat {
            insertions: 3,
            deletions: 1,
            files: 2,
        },
        DiffStat {
            insertions: 1,
            deletions: 2,
            files: 2,
        },
        DiffStat {
            insertions: 2,
            deletions: 0,
            files: 1,
        },
    ];
    assert_eq!(diffs, want);
}

/// Files with the same contents can have different attributes. Each counts by its own path's
/// attributes, as in git, so a binary file does not make its twin binary.
#[test]
fn identical_contents_follow_each_paths_attributes() {
    let mut b = RepoBuilder::new();
    b.commit(
        ada()
            .file(".gitattributes", "*.bin binary\n")
            .file("a.bin", "x\n")
            .file("b.txt", "x\n"),
    );
    let (_dir, repo) = b.open();

    // git log --numstat: `1 0 .gitattributes`, `- - a.bin`, `1 0 b.txt`.
    let want = DiffStat {
        insertions: 2,
        deletions: 0,
        files: 3,
    };
    assert_eq!(sum(&read(&repo, Some("HEAD"))), (1, want));
}

/// A file whose contents the repository lacks fails the read rather than counting as nothing. A
/// modification stops gix's tree walk partway, which is the harder case.
#[test]
fn a_missing_file_fails_naming_the_commit_and_path() {
    let mut b = RepoBuilder::new();
    let base = b.commit(ada().file("a.txt", "a\n"));
    let commit = b.commit(ada().parent(base).file_id("a.txt", missing_blob()));
    let (_dir, repo) = b.open();

    let err = repo.read(Some("HEAD"), |_| true, false).unwrap_err();
    let commit = commit.to_string();
    assert!(
        matches!(&err, git_stats::Error::ReadBlob { commit: c, path, .. }
            if *c == commit && path == "a.txt"),
        "{err:?}"
    );
}

/// In a partial clone, a failed read points at the missing objects, whether a file's own diff or
/// gix's rename detection reads the missing file first. git marks a partial clone with a promisor
/// remote, or with `extensions.partialClone` in older versions.
#[test]
fn a_partial_clone_names_itself_when_objects_are_missing() {
    let partial_clone_error = |marker: (&str, &str), second: Commit| {
        let mut b = RepoBuilder::new();
        b.config(marker.0, marker.1);
        let base = b.commit(ada().file("a.txt", "1\n2\n3\n"));
        b.commit(second.parent(base));
        let (_dir, repo) = b.open();
        repo.read(Some("HEAD"), |_| true, false).unwrap_err()
    };
    let promisor = ("remote.origin.promisor", "true");
    let modified = partial_clone_error(promisor, ada().file_id("a.txt", missing_blob()));
    // Deleting `a.txt` and adding `b.txt` makes rename detection read `b.txt`.
    let renamed = partial_clone_error(promisor, ada().file_id("b.txt", missing_blob()));
    let older = partial_clone_error(
        ("extensions.partialClone", "origin"),
        ada().file_id("a.txt", missing_blob()),
    );

    for err in [modified, renamed, older] {
        assert!(matches!(err, git_stats::Error::PartialClone(_)), "{err:?}");
    }

    let disabled = partial_clone_error(
        ("remote.origin.promisor", "false"),
        ada().file_id("a.txt", missing_blob()),
    );
    assert!(
        matches!(disabled, git_stats::Error::ReadBlob { .. }),
        "{disabled:?}"
    );
}

/// A tree entry with a null object ID is corrupt, and git fails to read it. gix would read it as an
/// empty file, so without a check it would count as an empty file or panic.
#[test]
fn a_null_object_id_fails_like_git() {
    let null = ObjectId::from_hex(b"0000000000000000000000000000000000000000").unwrap();
    let mut b = RepoBuilder::new();
    let base = b.commit(ada().file("a.txt", "a\n"));
    b.commit(
        ada()
            .parent(base)
            .file("a.txt", "a\n")
            .file_id("z.txt", null),
    );
    let (_dir, repo) = b.open();

    let err = repo.read(Some("HEAD"), |_| true, false).unwrap_err();
    assert!(
        matches!(&err, git_stats::Error::ReadBlob { path, .. } if path == "z.txt"),
        "{err:?}"
    );
}

/// An arbitrary blob ID that no test repository holds.
fn missing_blob() -> ObjectId {
    ObjectId::from_hex(b"3333333333333333333333333333333333333333").unwrap()
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
    let authors: Vec<Contributor> = read(&repo, Some("HEAD"))
        .into_iter()
        .map(|(meta, _)| meta.author)
        .collect();

    // Newest first. The old identity must not leak, and the unmapped Author passes through.
    let new = who("New Name", "new@example.com");
    let other = who("Other", "other@example.com");
    assert_eq!(authors, [other, new.clone(), new]);
}

#[test]
fn mailmap_folds_reviewer_identities() {
    let (_dir, repo) = mailmapped();
    let commits = repo.read(Some("HEAD"), |_| true, true).unwrap();

    let reviewers = commits.reviewers.unwrap().concat();
    assert_eq!(reviewers, [who("New Name", "new@example.com")]);
}
