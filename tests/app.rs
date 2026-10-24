//! Application tests: filters, Contributor keys, and the two tables, over repositories that
//! [`RepoBuilder`] writes.

mod common;

use git_stats::logic::table::Counts;
use git_stats::model::Options;
use git_stats::repo::Repo;
use tempfile::TempDir;

use common::{Commit, RepoBuilder, author, has_author, options, report, reviewer, run, total};

/// One commit per `(name, email, time)`, each on top of the last.
fn linear(authors: &[(&str, &str, i64)]) -> (TempDir, Repo) {
    let mut b = RepoBuilder::new();
    let mut parent = None;
    for (i, &(name, email, time)) in authors.iter().enumerate() {
        let mut commit = Commit::default()
            .author(name, email)
            .time(time)
            .file("a.txt", format!("{i}\n"));
        if let Some(parent) = parent {
            commit = commit.parent(parent);
        }
        parent = Some(b.commit(commit));
    }
    b.open()
}

/// Three commits across two Authors, with spaces in their names.
fn two_author_repo() -> (TempDir, Repo) {
    let mut b = RepoBuilder::new();
    let ada = |c: Commit| c.author("Ada Lovelace", "ada@x");
    let c1 = b.commit(ada(Commit::default()).file("a.txt", "1\n2\n3\n"));
    // Rewrites one line of a.txt and adds b.txt: 2 files, +2, -1.
    let c2 = b.commit(
        ada(Commit::default())
            .parent(c1)
            .file("a.txt", "1\n2\nx\n")
            .file("b.txt", "b\n"),
    );
    b.commit(
        Commit::default()
            .author("Bob Ross", "bob@x")
            .parent(c2)
            .file("a.txt", "1\n2\nx\n")
            .file("b.txt", "b\n")
            .file("c.txt", "c\n"),
    );
    b.open()
}

/// A commit by Ada in 1970 that Rev Iewer reviewed.
fn reviewed_repo() -> (TempDir, Repo) {
    let mut b = RepoBuilder::new();
    b.commit(
        Commit::default()
            .author("Ada", "ada@x")
            .time(100)
            .message("change\n\nReviewed-by: Rev Iewer <rev@x>\n")
            .file("a.txt", "a\n"),
    );
    b.open()
}

#[test]
fn sums_per_author_insertions_deletions_and_net() {
    let (_dir, repo) = two_author_repo();
    let r = report(&repo, &options(None));
    let ada = author(&r, "Ada Lovelace").counts;
    let ada_counts = Counts {
        commits: 2,
        files: 3,
        insertions: 5,
        deletions: 1,
    };
    assert_eq!((ada, ada.net()), (ada_counts, 4), "{r:#?}");
    let bob = author(&r, "Bob Ross").counts;
    let bob_counts = Counts {
        commits: 1,
        files: 1,
        insertions: 1,
        deletions: 0,
    };
    assert_eq!((bob, bob.net()), (bob_counts, 1), "{r:#?}");
}

#[test]
fn totals_cover_all_authors() {
    let (_dir, repo) = two_author_repo();
    let r = report(&repo, &options(None));
    let want = Counts {
        commits: 3,
        files: 4,
        insertions: 6,
        deletions: 1,
    };
    assert_eq!(total(&r), want, "{r:#?}");
}

#[test]
fn since_filters_out_older_commits() {
    // Times are unix seconds for 2020-01-01 and 2024-01-01.
    let (_dir, repo) = linear(&[
        ("Olda", "olda@x", 1_577_836_800),
        ("Newman", "newman@x", 1_704_067_200),
    ]);
    let mut opts = options(None);
    opts.since = Some("2022-01-01".to_string());

    let r = report(&repo, &opts);
    assert!(
        has_author(&r, "Newman"),
        "newer commit should remain: {r:#?}"
    );
    assert!(
        !has_author(&r, "Olda"),
        "older commit should be filtered: {r:#?}"
    );
}

#[test]
fn author_filter_keeps_only_matching_authors() {
    let (_dir, repo) = linear(&[("Alice", "alice@x", 100), ("Bob", "bob@x", 200)]);
    let mut opts = options(None);
    opts.authors = vec!["Alice".to_string()];

    let r = report(&repo, &opts);
    assert!(
        has_author(&r, "Alice"),
        "matching author should remain: {r:#?}"
    );
    assert!(
        !has_author(&r, "Bob"),
        "non-matching author should be gone: {r:#?}"
    );
}

#[test]
fn filter_validation_precedes_the_walk() {
    let (_dir, repo) = linear(&[("Ada", "ada@x", 100)]);
    let mut opts = options(Some("no-such-ref..HEAD"));
    opts.since = Some("not-a-date".to_string());

    // Both the range and the date are bad. The date must win: filters are cheap to validate and
    // should error before a potentially expensive walk of the whole range even starts.
    let err = run(&repo, &opts).unwrap_err();
    assert!(
        matches!(err, git_stats::Error::InvalidDate { .. }),
        "expected InvalidDate before any range resolution, got: {err:?}"
    );
}

#[test]
fn email_flag_splits_same_name_distinct_emails() {
    let (_dir, repo) = linear(&[("Luke", "luke@a", 100), ("Luke", "luke@b", 200)]);

    // Without --email, the two identities collapse into one "Luke" row.
    let merged = report(&repo, &options(None));
    assert_eq!(
        merged.stats.as_ref().unwrap().rows().len(),
        1,
        "{merged:#?}"
    );
    assert_eq!(author(&merged, "Luke").counts.commits, 2, "{merged:#?}");

    // With --email, they become two rows keyed by `Name <email>`.
    let mut with_email = options(None);
    with_email.email = true;
    let split = report(&repo, &with_email);
    assert_eq!(
        author(&split, "Luke <luke@a>").counts.commits,
        1,
        "{split:#?}"
    );
    assert_eq!(
        author(&split, "Luke <luke@b>").counts.commits,
        1,
        "{split:#?}"
    );
}

#[test]
fn reviews_ignore_the_author_and_date_filters() {
    let (_dir, repo) = reviewed_repo();
    let mut opts = options(None);
    opts.reviews = true;
    opts.authors = vec!["matches-nobody".to_string()];
    opts.since = Some("2022-01-01".to_string());

    let r = report(&repo, &opts);

    assert_eq!(r.stats, None, "{r:#?}");
    assert_eq!(reviewer(&r, "Rev Iewer").commits, 1, "{r:#?}");
}

#[test]
fn reviews_are_left_out_unless_requested() {
    let (_dir, repo) = reviewed_repo();

    let r = report(&repo, &options(None));

    assert!(r.reviews.is_empty(), "{r:#?}");
}

#[test]
fn reverse_flips_sort_order() {
    let (_dir, repo) = linear(&[
        ("Ada", "ada@x", 100),
        ("Ada", "ada@x", 200),
        ("Bob", "bob@x", 300),
    ]);
    let names = |opts: &Options| -> Vec<String> {
        let stats = report(&repo, opts).stats.unwrap();
        stats.rows().iter().map(|s| s.key.clone()).collect()
    };

    // Descending by commits: Ada (2) precedes Bob (1).
    assert_eq!(names(&options(None)), ["Ada", "Bob"]);

    let mut reversed = options(None);
    reversed.reverse = true;
    assert_eq!(names(&reversed), ["Bob", "Ada"]);
}
