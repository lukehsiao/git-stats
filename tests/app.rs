//! Application-level tests driven by a nulled [`Repo`]. They exercise the real
//! logic and coordination code end to end, with only the gix I/O neutralized.

use git_stats::app;
use git_stats::model::{Author, CommitMeta, DiffStat, Options, SortBy, Trailer};
use git_stats::repo::{NulledCommit, Repo};
use hegel::generators;

fn commit(name: &str, email: &str, time: i64, diff: DiffStat) -> NulledCommit {
    NulledCommit {
        meta: CommitMeta {
            author: Author {
                name: name.to_string(),
                email: email.to_string(),
            },
            time_seconds: time,
            trailers: Vec::new(),
        },
        diff,
        is_merge: false,
    }
}

fn diff(insertions: u64, deletions: u64, files: u64) -> DiffStat {
    DiffStat {
        insertions,
        deletions,
        files,
    }
}

fn options() -> Options {
    Options {
        range: "HEAD".to_string(),
        email: false,
        reviews: false,
        sort: SortBy::Commits,
        reverse: false,
        authors: Vec::new(),
        since: None,
        until: None,
    }
}

/// Run with color disabled, mirroring the plain output a non-tty (e.g. a pipe)
/// receives, so assertions see stable text.
fn report(repo: &Repo, opts: &Options) -> String {
    yansi::disable();
    app::run(repo, opts).unwrap()
}

/// The output line for a given leading label (an author or "Total").
fn row<'a>(out: &'a str, label: &str) -> &'a str {
    out.lines()
        .find(|l| l.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("no row starting with {label:?} in:\n{out}"))
}

/// Three commits across two authors (with spaces in their names), used by the
/// aggregation tests below.
fn two_author_repo() -> Repo {
    Repo::create_null(vec![
        commit("Ada Lovelace", "ada@x", 100, diff(10, 2, 3)),
        commit("Ada Lovelace", "ada@x", 200, diff(5, 5, 1)),
        commit("Bob Ross", "bob@x", 300, diff(1, 0, 1)),
    ])
}

#[test]
fn sums_per_author_insertions_deletions_and_net() {
    let out = report(&two_author_repo(), &options());
    // Ada Lovelace: 2 commits, 4 files, +15 / -7, net +8.
    let ada = row(&out, "Ada Lovelace");
    assert!(
        ada.contains("+15") && ada.contains("-7") && ada.contains("+8"),
        "ada row should show +15 / -7 / +8: {ada}"
    );
}

#[test]
fn appends_a_total_row_over_all_authors() {
    let out = report(&two_author_repo(), &options());
    // Total: 3 commits, +16 / -7, net +9.
    let total = row(&out, "Total");
    assert!(
        total.contains("+16") && total.contains("+9"),
        "total row should show +16 insertions and +9 net: {total}"
    );
}

#[test]
fn since_filters_out_older_commits() {
    // Times are unix seconds for 2020-01-01 and 2024-01-01.
    let repo = Repo::create_null(vec![
        commit("Olda", "olda@x", 1_577_836_800, diff(1, 0, 1)),
        commit("Newman", "newman@x", 1_704_067_200, diff(1, 0, 1)),
    ]);
    let mut opts = options();
    opts.since = Some("2022-01-01".to_string());

    let out = report(&repo, &opts);
    assert!(out.contains("Newman"), "newer commit should remain:\n{out}");
    assert!(
        !out.contains("Olda"),
        "older commit should be filtered:\n{out}"
    );
}

#[test]
fn author_filter_keeps_only_matching_authors() {
    let repo = Repo::create_null(vec![
        commit("Alice", "alice@x", 100, diff(1, 0, 1)),
        commit("Bob", "bob@x", 200, diff(1, 0, 1)),
    ]);
    let mut opts = options();
    opts.authors = vec!["Alice".to_string()];

    let out = report(&repo, &opts);
    assert!(
        out.contains("Alice"),
        "matching author should remain:\n{out}"
    );
    assert!(
        !out.contains("Bob"),
        "non-matching author should be gone:\n{out}"
    );
}

#[test]
fn empty_repo_renders_nothing() {
    let repo = Repo::create_null(vec![]);
    assert_eq!(report(&repo, &options()), "");
}

#[test]
fn merge_commits_count_but_contribute_no_lines() {
    let mut merge = commit("Ada", "ada@x", 100, diff(999, 999, 9));
    merge.is_merge = true;
    let repo = Repo::create_null(vec![merge, commit("Ada", "ada@x", 200, diff(4, 1, 2))]);

    let out = report(&repo, &options());
    let ada = row(&out, "Ada");
    // Only the non-merge contributes lines: +4 / -1, net +3.
    assert!(ada.contains("+4"), "ada row: {ada}");
    assert!(ada.contains("-1"), "ada row: {ada}");
    assert!(ada.contains("+3"), "ada row: {ada}");
    assert!(
        !out.contains("999"),
        "merge stats leaked into output:\n{out}"
    );
}

#[test]
fn email_flag_splits_same_name_distinct_emails() {
    let commits = vec![
        commit("Luke", "luke@a", 100, diff(1, 0, 1)),
        commit("Luke", "luke@b", 200, diff(2, 0, 1)),
    ];

    // Without --email, the two identities collapse into one "Luke" row.
    let merged = report(&Repo::create_null(commits.clone()), &options());
    assert!(row(&merged, "Luke").contains("+3"), "merged: {merged}");
    assert!(
        !merged.contains("luke@a"),
        "email leaked without flag: {merged}"
    );

    // With --email, they become two rows keyed by `Name <email>`.
    let mut with_email = options();
    with_email.email = true;
    let split = report(&Repo::create_null(commits), &with_email);
    assert!(split.contains("luke@a"), "split: {split}");
    assert!(split.contains("luke@b"), "split: {split}");
}

#[test]
fn reviews_credit_a_reviewer_once_per_commit() {
    let mut reviewed = commit("Ada", "ada@x", 100, diff(1, 0, 1));
    reviewed.meta.trailers = vec![
        Trailer {
            token: "Reviewed-by".to_string(),
            value: "Rev Iewer <rev@x>".to_string(),
        },
        Trailer {
            token: "Tested-by".to_string(),
            value: "Rev Iewer <rev@x>".to_string(),
        },
        // Not a review/test trailer; must be ignored.
        Trailer {
            token: "Signed-off-by".to_string(),
            value: "Sole Author <sole@x>".to_string(),
        },
    ];

    let mut with_reviews = options();
    with_reviews.reviews = true;
    let out = report(&Repo::create_null(vec![reviewed]), &with_reviews);

    assert!(out.contains("Reviewer/Tester"), "no reviews table:\n{out}");
    // Two matching trailers on one commit still credit the reviewer once.
    let rev = row(&out, "Rev Iewer");
    assert_eq!(rev.split_whitespace().last(), Some("1"), "rev row: {rev}");
    // The Signed-off-by trailer is not a review token, so it is not counted.
    assert!(
        !out.contains("Sole Author"),
        "non-review trailer leaked:\n{out}"
    );
}

#[test]
fn reviews_alone_render_without_a_leading_blank_line() {
    // Reviews ignore the author filter by design, so a filter that matches
    // nothing empties the stats table while the reviews table still renders.
    let mut reviewed = commit("Ada", "ada@x", 100, diff(1, 0, 1));
    reviewed.meta.trailers = vec![Trailer {
        token: "Reviewed-by".to_string(),
        value: "Rev Iewer <rev@x>".to_string(),
    }];
    let mut opts = options();
    opts.reviews = true;
    opts.authors = vec!["matches-nobody".to_string()];

    let out = report(&Repo::create_null(vec![reviewed]), &opts);

    assert!(
        out.starts_with("Reviewer/Tester"),
        "reviews table should start at the first line:\n{out:?}"
    );
}

/// For any commit set and display options, the report never opens with a
/// blank line, and a non-empty report ends with exactly one newline. The
/// separator between the stats and reviews tables must only appear when both
/// render; the nulled [`Repo`] lets the property cover the whole app layer.
#[hegel::test]
fn report_framing_is_stable(tc: hegel::TestCase) {
    let n = tc.draw(generators::integers::<usize>().max_value(8));
    let mut commits = Vec::with_capacity(n);
    for _ in 0..n {
        let who = tc.draw(generators::sampled_from(vec!["Ada Lovelace", "Bob"]));
        let mut c = commit(who, "x@x", 100, diff(1, 0, 1));
        if tc.draw(generators::booleans()) {
            c.meta.trailers = vec![Trailer {
                token: "Reviewed-by".to_string(),
                value: "Rev Iewer <rev@x>".to_string(),
            }];
        }
        commits.push(c);
    }
    let mut opts = options();
    opts.reviews = tc.draw(generators::booleans());
    opts.email = tc.draw(generators::booleans());
    // Sometimes filter every commit out, so only the reviews table renders.
    if tc.draw(generators::booleans()) {
        opts.authors = vec!["matches-nobody".to_string()];
    }

    let out = report(&Repo::create_null(commits), &opts);

    assert!(!out.starts_with('\n'), "report opens blank: {out:?}");
    if !out.is_empty() {
        assert!(
            out.ends_with('\n') && !out.ends_with("\n\n"),
            "report should end with exactly one newline: {out:?}"
        );
    }
}

#[test]
fn reverse_flips_sort_order() {
    let repo = Repo::create_null(vec![
        commit("Ada", "ada@x", 100, diff(1, 0, 1)),
        commit("Ada", "ada@x", 200, diff(1, 0, 1)),
        commit("Bob", "bob@x", 300, diff(1, 0, 1)),
    ]);

    // Descending by commits: Ada (2) precedes Bob (1).
    let desc = report(&repo, &options());
    assert!(
        desc.find("Ada").unwrap() < desc.find("Bob").unwrap(),
        "descending order wrong:\n{desc}"
    );

    // Reversed: Bob precedes Ada (the Total row is still appended last).
    let mut reversed = options();
    reversed.reverse = true;
    let asc = report(&repo, &reversed);
    assert!(
        asc.find("Bob").unwrap() < asc.find("Ada").unwrap(),
        "reverse failed:\n{asc}"
    );
}
