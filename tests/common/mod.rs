//! Helpers shared by the integration test crates.

// Each test crate compiles its own copy of this module and uses a different subset of it, so every
// crate would otherwise warn about the rest.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use gix::ObjectId;
use gix::objs::tree::EntryKind;
use tempfile::TempDir;

use clap::Parser as _;
use git_stats::app;
use git_stats::cli::Cli;
use git_stats::error::Result;
use git_stats::logic::table::{AuthorRow, Counts, Report, Review};
use git_stats::model::{CommitMeta, DiffStat};
use git_stats::repo::{Repo, Vcs};

/// An arbitrary but fixed `now` for relative dates. Its odd offset makes a test that depends on the
/// time zone by accident stand out.
fn now() -> gix::date::Zoned {
    "2001-02-03T04:05:06+05:45[+05:45]".parse().unwrap()
}

/// Run the application on the command line `git-stats <args>`, as of [`now`].
pub fn run(repo: &Repo, args: &[&str]) -> Result<Report> {
    let command_line = std::iter::once("git-stats").chain(args.iter().copied());
    app::run(repo, &Cli::try_parse_from(command_line).unwrap(), &now())
}

pub fn report(repo: &Repo, args: &[&str]) -> Report {
    run(repo, args).unwrap()
}

/// Every commit in `range`, all kept, each with its numstat.
pub fn read(repo: &Repo, range: Option<&str>) -> Vec<(CommitMeta, DiffStat)> {
    repo.read(range, |_| true, false).unwrap().kept
}

/// How many `commits` there are, and their numstats summed.
pub fn sum(commits: &[(CommitMeta, DiffStat)]) -> (usize, DiffStat) {
    let total = commits
        .iter()
        .fold(DiffStat::default(), |t, (_, d)| DiffStat {
            insertions: t.insertions + d.insertions,
            deletions: t.deletions + d.deletions,
            files: t.files + d.files,
        });
    (commits.len(), total)
}

/// The stats table's Author rows, or none without a stats table.
fn rows(report: &Report) -> &[AuthorRow] {
    report.stats.as_ref().map_or(&[], |t| t.rows())
}

/// The stats table row whose Contributor key is exactly `key`.
pub fn author<'a>(report: &'a Report, key: &str) -> &'a AuthorRow {
    rows(report)
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no author {key:?} in {report:#?}"))
}

/// Whether the stats table has a row whose Contributor key is exactly `key`.
pub fn has_author(report: &Report, key: &str) -> bool {
    rows(report).iter().any(|r| r.key == key)
}

/// The Total row's counts.
pub fn total(report: &Report) -> Counts {
    report
        .stats
        .as_ref()
        .unwrap_or_else(|| panic!("no stats table in {report:#?}"))
        .total()
}

/// The reviews table row whose Contributor key is exactly `key`.
pub fn reviewer<'a>(report: &'a Report, key: &str) -> &'a Review {
    report
        .reviews
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no reviewer {key:?} in {report:#?}"))
}

/// Run git in `dir` and return its trimmed stdout. Host git config stays out because
/// `.cargo/config.toml` points git at the test global config and disables the system one.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git should be installed");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Whether a usable `git` binary is on PATH; tests skip gracefully without one.
pub fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

/// The identity, time, and message of a fixture commit that a test does not set. They are
/// obviously fake, so a test that depends on them by accident stands out.
const IRRELEVANT_NAME: &str = "Irrelevant Author";
const IRRELEVANT_EMAIL: &str = "irrelevant@example.invalid";
const IRRELEVANT_TIME: i64 = 1_000_000_000;
const IRRELEVANT_MESSAGE: &str = "irrelevant message\n";

/// A commit for [`RepoBuilder::commit`]. Its files are the whole tree, so its numstat is the real
/// diff against its first parent.
pub struct Commit {
    name: String,
    email: String,
    time: i64,
    parents: Vec<ObjectId>,
    message: String,
    entries: Vec<(String, Entry)>,
}

enum Entry {
    Blob(Vec<u8>),
    /// An entry of this kind that points at this object, which need not exist.
    Object(EntryKind, ObjectId),
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            name: IRRELEVANT_NAME.to_string(),
            email: IRRELEVANT_EMAIL.to_string(),
            time: IRRELEVANT_TIME,
            parents: Vec::new(),
            message: IRRELEVANT_MESSAGE.to_string(),
            entries: Vec::new(),
        }
    }
}

impl Commit {
    /// Set the author and the committer.
    #[must_use]
    pub fn author(mut self, name: &str, email: &str) -> Self {
        self.name = name.to_string();
        self.email = email.to_string();
        self
    }

    /// Set the author and committer time, in seconds since the Unix epoch.
    #[must_use]
    pub fn time(mut self, seconds: i64) -> Self {
        self.time = seconds;
        self
    }

    /// Add a parent. The first one added is the one the numstat diffs against.
    #[must_use]
    pub fn parent(mut self, parent: ObjectId) -> Self {
        self.parents.push(parent);
        self
    }

    #[must_use]
    pub fn message(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }

    #[must_use]
    pub fn file(mut self, path: &str, contents: impl AsRef<[u8]>) -> Self {
        let blob = Entry::Blob(contents.as_ref().to_vec());
        self.entries.push((path.to_string(), blob));
        self
    }

    /// Add a submodule entry that points at `commit`, which need not exist.
    #[must_use]
    pub fn gitlink(mut self, path: &str, commit: ObjectId) -> Self {
        let entry = Entry::Object(EntryKind::Commit, commit);
        self.entries.push((path.to_string(), entry));
        self
    }

    /// Add a file entry that points at `blob`, which need not exist, as in a partial clone.
    #[must_use]
    pub fn file_id(mut self, path: &str, blob: ObjectId) -> Self {
        let entry = Entry::Object(EntryKind::Blob, blob);
        self.entries.push((path.to_string(), entry));
        self
    }
}

/// Writes a git repository into a temporary directory with gix, so tests describe history as git
/// stores it and need no `git` binary.
pub struct RepoBuilder {
    dir: TempDir,
    repo: gix::Repository,
    head: Option<ObjectId>,
}

impl RepoBuilder {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let repo = gix::init(dir.path()).unwrap();
        Self {
            dir,
            repo,
            head: None,
        }
    }

    /// Write `commit` and return its ID.
    pub fn commit(&mut self, commit: Commit) -> ObjectId {
        let tree = self.write_tree(commit.entries);
        let signature = gix::actor::Signature {
            name: commit.name.into(),
            email: commit.email.into(),
            time: gix::date::Time::new(commit.time, 0),
        };
        let id = self
            .repo
            .write_object(gix::objs::Commit {
                tree,
                parents: commit.parents.into(),
                author: signature.clone(),
                committer: signature,
                encoding: None,
                message: commit.message.into(),
                extra_headers: Vec::new(),
            })
            .unwrap()
            .detach();
        self.head = Some(id);
        id
    }

    /// Write the tree of `entries` and return its ID. A path with a `/` puts its entry in a
    /// subdirectory.
    fn write_tree(&mut self, entries: Vec<(String, Entry)>) -> ObjectId {
        let mut tree = gix::objs::Tree::empty();
        let mut dirs: BTreeMap<String, Vec<(String, Entry)>> = BTreeMap::new();
        for (path, entry) in entries {
            if let Some((dir, rest)) = path.split_once('/') {
                dirs.entry(dir.to_string())
                    .or_default()
                    .push((rest.to_string(), entry));
                continue;
            }
            let (kind, oid) = match entry {
                Entry::Blob(bytes) => (
                    EntryKind::Blob,
                    self.repo.write_blob(bytes).unwrap().detach(),
                ),
                Entry::Object(kind, id) => (kind, id),
            };
            tree.entries.push(gix::objs::tree::Entry {
                mode: kind.into(),
                filename: path.into(),
                oid,
            });
        }
        for (dir, entries) in dirs {
            let oid = self.write_tree(entries);
            tree.entries.push(gix::objs::tree::Entry {
                mode: EntryKind::Tree.into(),
                filename: dir.into(),
                oid,
            });
        }
        // git requires tree entries in its own name order.
        tree.entries.sort();
        self.repo.write_object(&tree).unwrap().detach()
    }

    pub fn branch(&mut self, name: &str, id: ObjectId) {
        self.write_ref(&format!("refs/heads/{name}"), id);
    }

    pub fn lightweight_tag(&mut self, name: &str, id: ObjectId) {
        self.write_ref(&format!("refs/tags/{name}"), id);
    }

    /// Write an annotated tag of `target`, a commit or another tag, and return the tag object.
    pub fn annotated_tag(&mut self, name: &str, target: ObjectId) -> ObjectId {
        let tag = self
            .repo
            .write_object(gix::objs::Tag {
                target,
                target_kind: self.repo.find_object(target).unwrap().kind,
                name: name.into(),
                tagger: None,
                message: "release\n".into(),
                signature: None,
            })
            .unwrap()
            .detach();
        self.lightweight_tag(name, tag);
        tag
    }

    /// Write a loose ref file. gix's own ref edits also write a reflog, which needs a committer
    /// identity that the isolated test config does not have.
    fn write_ref(&self, name: &str, id: ObjectId) {
        let path = self.repo.git_dir().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{id}\n")).unwrap();
    }

    /// Mark `boundary` as the commits whose parents a shallow clone lacks.
    pub fn shallow(&mut self, boundary: &[ObjectId]) {
        let mut file = std::fs::File::create(self.repo.shallow_file().unwrap()).unwrap();
        for id in boundary {
            writeln!(file, "{id}").unwrap();
        }
    }

    /// Write `.mailmap` into the worktree without committing it, so it adds no changed file.
    pub fn mailmap(&mut self, contents: &str) {
        std::fs::write(self.dir.path().join(".mailmap"), contents).unwrap();
    }

    /// Set `section.key`, or `section.subsection.key`, in the repository's own config.
    pub fn config(&mut self, key: &str, value: &str) {
        let (section, key) = key.split_once('.').unwrap();
        let (header, key) = match key.rsplit_once('.') {
            Some((subsection, key)) => (format!("{section} \"{subsection}\""), key),
            None => (section.to_string(), key),
        };
        let mut config = std::fs::OpenOptions::new()
            .append(true)
            .open(self.repo.git_dir().join("config"))
            .unwrap();
        write!(config, "[{header}]\n\t{key} = {value}\n").unwrap();
    }

    /// Point `HEAD` at the last commit written and open the repository as git-stats does. The
    /// directory lives as long as the returned [`TempDir`].
    pub fn open(self) -> (TempDir, Repo) {
        if let Some(head) = self.head {
            self.write_ref("HEAD", head);
        }
        let repo = Repo::open(self.dir.path(), Vcs::Git).unwrap();
        (self.dir, repo)
    }
}
