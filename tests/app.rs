//! Application-level tests driven by a nulled [`Repo`]. They exercise the real logic and
//! coordination code end to end, with only the gix I/O neutralized.

mod common;

use git_stats::model::{CommitMeta, Contributor, DiffStat, Options};
use git_stats::repo::{NulledCommit, Repo};

use common::{author, counts, has_author, options, report, reviewer, run, total};

fn commit(name: &str, email: &str, time: i64, diff: DiffStat) -> NulledCommit {
    NulledCommit {
        meta: CommitMeta {
            author: Contributor {
                name: name.to_string(),
                email: email.to_string(),
            },
            time_seconds: time,
            reviewers: Vec::new(),
        },
        diff,
    }
}

fn diff(insertions: u64, deletions: u64, files: u64) -> DiffStat {
    DiffStat {
        insertions,
        deletions,
        files,
    }
}

/// A commit by Ada that Rev Iewer reviewed.
fn reviewed_commit() -> NulledCommit {
    let mut c = commit("Ada", "ada@x", 100, diff(1, 0, 1));
    c.meta.reviewers = vec![Contributor {
        name: "Rev Iewer".to_string(),
        email: "rev@x".to_string(),
    }];
    c
}

/// Three commits across two authors (with spaces in their names), used by the aggregation tests
/// below.
fn two_author_repo() -> Repo {
    Repo::create_null(vec![
        commit("Ada Lovelace", "ada@x", 100, diff(10, 2, 3)),
        commit("Ada Lovelace", "ada@x", 200, diff(5, 5, 1)),
        commit("Bob Ross", "bob@x", 300, diff(1, 0, 1)),
    ])
}

#[test]
fn sums_per_author_insertions_deletions_and_net() {
    let r = report(&two_author_repo(), &options(None));
    assert_eq!(
        counts(author(&r, "Ada Lovelace")),
        (2, 4, 15, 7, 8),
        "{r:#?}"
    );
    assert_eq!(counts(author(&r, "Bob Ross")), (1, 1, 1, 0, 1), "{r:#?}");
}

#[test]
fn totals_cover_all_authors() {
    let r = report(&two_author_repo(), &options(None));
    assert_eq!(counts(total(&r)), (3, 5, 16, 7, 9), "{r:#?}");
}

#[test]
fn since_filters_out_older_commits() {
    // Times are unix seconds for 2020-01-01 and 2024-01-01.
    let repo = Repo::create_null(vec![
        commit("Olda", "olda@x", 1_577_836_800, diff(1, 0, 1)),
        commit("Newman", "newman@x", 1_704_067_200, diff(1, 0, 1)),
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
    let repo = Repo::create_null(vec![
        commit("Alice", "alice@x", 100, diff(1, 0, 1)),
        commit("Bob", "bob@x", 200, diff(1, 0, 1)),
    ]);
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
fn empty_repo_has_nothing_to_show() {
    let mut opts = options(None);
    opts.reviews = true;
    let r = report(&Repo::create_null(vec![]), &opts);
    assert_eq!(r.stats, None);
    assert!(r.reviews.is_empty(), "{r:#?}");
}

#[test]
fn invalid_author_pattern_is_a_typed_error() {
    let repo = Repo::create_null(vec![commit("Ada", "ada@x", 100, diff(1, 0, 1))]);
    let mut opts = options(None);
    opts.authors = vec!["[".to_string()];

    let err = run(&repo, &opts).unwrap_err();
    assert!(
        matches!(err, git_stats::Error::AuthorPattern(_)),
        "expected AuthorPattern, got: {err:?}"
    );
}

#[test]
fn email_flag_splits_same_name_distinct_emails() {
    let commits = vec![
        commit("Luke", "luke@a", 100, diff(1, 0, 1)),
        commit("Luke", "luke@b", 200, diff(2, 0, 1)),
    ];

    // Without --email, the two identities collapse into one "Luke" row.
    let merged = report(&Repo::create_null(commits.clone()), &options(None));
    assert_eq!(merged.stats.as_ref().unwrap().rows.len(), 1, "{merged:#?}");
    assert_eq!(author(&merged, "Luke").insertions, 3, "{merged:#?}");

    // With --email, they become two rows keyed by `Name <email>`.
    let mut with_email = options(None);
    with_email.email = true;
    let split = report(&Repo::create_null(commits), &with_email);
    assert_eq!(author(&split, "Luke <luke@a>").insertions, 1, "{split:#?}");
    assert_eq!(author(&split, "Luke <luke@b>").insertions, 2, "{split:#?}");
}

#[test]
fn reviews_ignore_the_author_and_date_filters() {
    let reviewed = reviewed_commit();
    let mut opts = options(None);
    opts.reviews = true;
    opts.authors = vec!["matches-nobody".to_string()];
    opts.since = Some("2022-01-01".to_string());

    let r = report(&Repo::create_null(vec![reviewed]), &opts);

    assert_eq!(r.stats, None, "{r:#?}");
    assert_eq!(reviewer(&r, "Rev Iewer").commits, 1, "{r:#?}");
}

#[test]
fn reviews_are_left_out_unless_requested() {
    let reviewed = reviewed_commit();

    let r = report(&Repo::create_null(vec![reviewed]), &options(None));

    assert!(r.reviews.is_empty(), "{r:#?}");
}

#[test]
fn reverse_flips_sort_order() {
    let repo = Repo::create_null(vec![
        commit("Ada", "ada@x", 100, diff(1, 0, 1)),
        commit("Ada", "ada@x", 200, diff(1, 0, 1)),
        commit("Bob", "bob@x", 300, diff(1, 0, 1)),
    ]);
    let names = |opts: &Options| -> Vec<String> {
        let r = report(&repo, opts);
        r.stats.unwrap().rows.into_iter().map(|s| s.key).collect()
    };

    // Descending by commits: Ada (2) precedes Bob (1).
    assert_eq!(names(&options(None)), ["Ada", "Bob"]);

    let mut reversed = options(None);
    reversed.reverse = true;
    assert_eq!(names(&reversed), ["Bob", "Ada"]);
}
