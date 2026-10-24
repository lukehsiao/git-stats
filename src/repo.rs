//! Infrastructure: the only module that talks to gix, the `jj` CLI, and the filesystem.
//!
//! [`Repo`] has two constructors. [`Repo::open`] reads a real repository; [`Repo::create_null`]
//! returns an in-memory stand-in driven by canned commits, so application and logic code can be
//! tested without touching git.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rayon::prelude::*;

use crate::error::{Error, Result};
use crate::model::{CommitMeta, Contributor, DiffStat, Vcs};

/// A commit yielded by [`Repo::walk`]: its logic-facing metadata plus an opaque handle the wrapper
/// uses to compute the numstat later.
pub struct WalkedCommit {
    pub meta: CommitMeta,
    is_merge: bool,
    handle: Handle,
}

impl WalkedCommit {
    /// Where the numstat comes from. A merge contributes nothing, matching `git log --numstat`, so
    /// it needs no diff.
    fn numstat_source(&self) -> Handle {
        if self.is_merge {
            Handle::Known(DiffStat::default())
        } else {
            self.handle
        }
    }
}

/// How to obtain a commit's numstat.
#[derive(Clone, Copy)]
enum Handle {
    /// gix diffs the commit against its first parent.
    Diff(gix::ObjectId),
    /// The numstat is already known: canned in a nulled repository, or computed by jj where a
    /// conflict makes the git trees meaningless.
    Known(DiffStat),
}

/// A canned commit used to build a nulled [`Repo`] in tests.
#[derive(Debug, Clone)]
pub struct NulledCommit {
    pub meta: CommitMeta,
    pub diff: DiffStat,
    pub is_merge: bool,
}

enum Backend {
    Real {
        // Boxed because a `ThreadSafeRepository` is far larger than the null variant.
        repo: Box<gix::ThreadSafeRepository>,
        revisions: Revisions,
    },
    Null(Vec<NulledCommit>),
}

/// Who selects the commits of a revision range. Either way, gix reads the selected commits from the
/// git object store.
enum Revisions {
    /// gix resolves a git revision range.
    Git,
    /// The `jj` CLI evaluates a revset in the workspace rooted at this path. jj stores every commit
    /// as a git object, so gix can read all of them.
    Jj(PathBuf),
}

/// A git repository or jj workspace, or a nulled stand-in for it.
pub struct Repo {
    backend: Backend,
}

impl Repo {
    /// Open the repository containing `path`.
    ///
    /// For git, this honors `GIT_DIR` and related environment variables first (git sets them when
    /// running hooks and for `--git-dir` invocations), then searches upward from `path`. For jj, it
    /// opens the git repository that backs the nearest jj workspace. [`Vcs::Auto`] picks git when
    /// `GIT_DIR` is set, and otherwise whichever of the two is nearer, with jj for a colocated
    /// workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no repository can be discovered, or if `jj` cannot run or does not
    /// report a git-backed repository.
    pub fn open(path: impl AsRef<Path>, vcs: Vcs) -> Result<Self> {
        let path = path.as_ref();
        let workspace = match vcs {
            Vcs::Git => None,
            Vcs::Auto if std::env::var_os("GIT_DIR").is_some() => None,
            Vcs::Auto => find_jj_workspace(path, true),
            Vcs::Jj => {
                Some(
                    find_jj_workspace(path, false).ok_or_else(|| Error::NoJjWorkspace {
                        path: std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
                    })?,
                )
            }
        };
        let Some(workspace) = workspace else {
            let repo = gix::ThreadSafeRepository::discover_with_environment_overrides(path)
                .map_err(|e| Error::OpenRepository(Box::new(e)))?;
            return Ok(Self {
                backend: Backend::Real {
                    repo: Box::new(repo),
                    revisions: Revisions::Git,
                },
            });
        };
        // Reading the store location needs no snapshot of the working copy. The revset walk
        // snapshots it later, before any commit is read.
        let git_dir = jj(
            &workspace,
            "git root",
            &["git", "root", "--ignore-working-copy"],
        )?;
        let git_dir = git_dir.strip_suffix('\n').unwrap_or(&git_dir);
        let mut repo = gix::open(git_dir).map_err(|e| Error::OpenRepository(Box::new(e)))?;
        // gix reads `.mailmap` and `.gitattributes` from the worktree of the git store. jj's own
        // store has none, and a colocated or external store's worktree may be another workspace. So
        // the workspace stands in for the worktree, as the checkout does for git.
        repo.set_workdir(workspace.clone())
            .map_err(|e| Error::OpenRepository(Box::new(e)))?;
        Ok(Self {
            backend: Backend::Real {
                repo: Box::new(repo.into_sync()),
                revisions: Revisions::Jj(workspace),
            },
        })
    }

    /// Build a nulled repository whose [`walk`](Self::walk) returns the given commits verbatim. The
    /// revision range is ignored, since range resolution is real-git behavior exercised separately
    /// by integration tests.
    #[must_use]
    pub fn create_null(commits: Vec<NulledCommit>) -> Self {
        Self {
            backend: Backend::Null(commits),
        }
    }

    /// Whether the repository is a shallow clone. Its truncated history makes every count differ
    /// from the full clone's, so callers may want to warn. Nulled repositories never are. A
    /// shallow-file path that cannot be represented natively counts as "not shallow", as the walk
    /// treats it.
    #[must_use]
    pub fn is_shallow(&self) -> bool {
        match &self.backend {
            Backend::Real { repo, .. } => repo.to_thread_local().is_shallow().unwrap_or(false),
            Backend::Null(_) => false,
        }
    }

    /// Read commit metadata for every commit in `range`: a git revision range, or a revset in a jj
    /// workspace. `None` means `HEAD` for git and `::@` for jj. Reviewers are parsed only when
    /// `need_reviewers` is set, since they feed only the reviews table and decoding every commit
    /// message is costly on large histories.
    ///
    /// # Errors
    ///
    /// Returns an error if the range cannot be resolved or a commit object cannot be read.
    pub fn walk(&self, range: Option<&str>, need_reviewers: bool) -> Result<Vec<WalkedCommit>> {
        match &self.backend {
            Backend::Real {
                repo: tsr,
                revisions,
            } => {
                let mut repo = tsr.to_thread_local();
                // Match `numstats`: an object cache lets the metadata read reuse commits the
                // traversal already decoded instead of re-reading packs.
                repo.object_cache_size_if_unset(8 * 1024 * 1024);
                match revisions {
                    Revisions::Git => walk_git(&repo, range.unwrap_or("HEAD"), need_reviewers),
                    Revisions::Jj(workspace) => {
                        walk_jj(&repo, workspace, range.unwrap_or("::@"), need_reviewers)
                    }
                }
            }
            Backend::Null(commits) => Ok(commits
                .iter()
                .map(|c| WalkedCommit {
                    meta: c.meta.clone(),
                    is_merge: c.is_merge,
                    handle: Handle::Known(c.diff),
                })
                .collect()),
        }
    }

    /// Compute the numstat for each given commit. Merge commits contribute nothing, matching `git
    /// log --numstat`'s default.
    ///
    /// # Errors
    ///
    /// Returns an error if a commit's tree or its parent's tree cannot be diffed.
    pub fn numstats(&self, commits: &[&WalkedCommit]) -> Result<Vec<DiffStat>> {
        match &self.backend {
            Backend::Real {
                repo: tsr,
                revisions,
            } => commits
                .par_iter()
                .map_init(
                    || Worker::new(tsr.to_thread_local(), revisions),
                    |worker, c| worker.numstat(c),
                )
                .collect(),
            Backend::Null(_) => Ok(commits
                .iter()
                .map(|c| match c.numstat_source() {
                    Handle::Known(diff) => diff,
                    // A nulled repository yields only known numstats.
                    Handle::Diff(_) => DiffStat::default(),
                })
                .collect()),
        }
    }
}

/// The root of the jj workspace containing `path`: the nearest ancestor with a `.jj` directory.
/// With `stop_at_git`, a nearer `.git` ends the search, so a git repository nested inside a jj
/// workspace stays git. A colocated workspace has both at its root and counts as jj.
fn find_jj_workspace(path: &Path, stop_at_git: bool) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    for dir in path.ancestors() {
        if dir.join(".jj").is_dir() {
            return Some(dir.to_path_buf());
        }
        if stop_at_git && dir.join(".git").exists() {
            return None;
        }
    }
    None
}

/// Run `jj` in `workspace` and return its stdout. `command` names the subcommand in errors. jj's
/// stderr passes through, so its own errors, hints, and warnings reach the user verbatim.
fn jj(workspace: &Path, command: &'static str, args: &[&str]) -> Result<String> {
    let out = Command::new("jj")
        .current_dir(workspace)
        // Keep stdout machine-readable whatever the user's jj config says. A wrapped line would
        // split a commit's fields across two lines.
        .args([
            "--no-pager",
            "--color=never",
            "--config",
            "ui.log-word-wrap=false",
        ])
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::SpawnJj)?;
    if !out.status.success() {
        return Err(Error::Jj {
            command,
            status: out.status,
        });
    }
    String::from_utf8(out.stdout).map_err(|e| Error::JjOutput {
        command,
        output: String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

/// Per-rayon-worker state for the real backend: a thread-local repository and two diff resource
/// caches built once and reused for every commit this worker handles. `gix::Tree::stats` would
/// instead build a fresh cache per call, which re-parses the on-disk index and reassembles the
/// attribute stack each time; on a large history that rebuild, not the diff itself, dominates the
/// runtime.
struct Worker<'a> {
    repo: gix::Repository,
    revisions: &'a Revisions,
    // Commits at the shallow boundary, whose parents exist in their headers but not in the object
    // database. `None` when the clone is not shallow.
    shallow: Option<gix::shallow::Commits>,
    // `for_each_to_obtain_tree_with_cache` holds one cache for the tree walk while each change's
    // line diff needs another, so a worker keeps two. They are built lazily because the cache
    // constructor is fallible and rayon's `map_init` initializer cannot return a `Result`.
    caches: Option<(gix::diff::blob::Platform, gix::diff::blob::Platform)>,
}

impl<'a> Worker<'a> {
    fn new(mut repo: gix::Repository, revisions: &'a Revisions) -> Self {
        // Per-thread object cache so repeated tree/blob reads during diffing hit memory instead of
        // the pack. 8 MiB comfortably holds a commit's trees.
        repo.object_cache_size_if_unset(8 * 1024 * 1024);
        // An unreadable shallow file degrades to "not shallow"; a boundary commit then surfaces its
        // missing parent as a read error below.
        let shallow = repo.shallow_commits().ok().flatten();
        Self {
            repo,
            revisions,
            shallow,
            caches: None,
        }
    }

    fn numstat(&mut self, commit: &WalkedCommit) -> Result<DiffStat> {
        let id = match commit.numstat_source() {
            Handle::Diff(id) => id,
            Handle::Known(diff) => return Ok(diff),
        };
        if self.caches.is_none() {
            let walk = diff_cache(&self.repo, self.revisions)?;
            let count = walk.clone();
            self.caches = Some((walk, count));
        }
        let Self {
            repo,
            shallow,
            caches,
            ..
        } = self;
        let (walk_cache, count_cache) = caches.as_mut().expect("initialized above");
        numstat_real(repo, walk_cache, count_cache, shallow.as_ref(), id)
    }
}

/// A diff resource cache with the `.gitattributes` of the checkout being read: from the index of a
/// git repository, as gix reads them, or from the files of a jj workspace.
fn diff_cache(repo: &gix::Repository, revisions: &Revisions) -> Result<gix::diff::blob::Platform> {
    let err = |e: gix::Error| Error::DiffStats(Box::new(e));
    match revisions {
        Revisions::Git => repo.diff_resource_cache_for_tree_diff().map_err(err),
        // The index and `HEAD` of jj's git store may be empty, or may belong to another workspace.
        // So attributes come only from the files in this workspace, which `Repo::open` made the
        // worktree.
        Revisions::Jj(_) => {
            let index = gix::index::State::new(repo.object_hash());
            let attributes = repo
                .attributes_only(
                    &index,
                    gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
                )
                .map_err(err)?
                .detach();
            gix::diff::resource_cache(
                repo,
                gix::diff::blob::pipeline::Mode::ToGit,
                attributes,
                gix::diff::blob::pipeline::WorktreeRoots::default(),
            )
            .map_err(err)
        }
    }
}

fn walk_git(
    repo: &gix::Repository,
    range: &str,
    need_reviewers: bool,
) -> Result<Vec<WalkedCommit>> {
    let (tips, hidden) = resolve_range(repo, range)?;
    let walk = repo
        .rev_walk(tips)
        .with_hidden(hidden)
        .all()
        .map_err(|e| Error::WalkRange {
            range: range.to_string(),
            source: Box::new(e),
        })?;
    // Stream the IDs so each commit is read right after the traversal decoded it, while it is still
    // in the object cache.
    let commits = walk.map(|info| {
        info.map(|info| (info.id, None))
            .map_err(|e| Error::ReadCommit(Box::new(e)))
    });
    read_commits(repo, commits, need_reviewers)
}

/// The `jj log` template for [`walk_jj`]: one line per commit with its ID. A commit that is
/// conflicted, or has a conflicted parent, also gets its file, insertion, and deletion counts. jj
/// stores a conflict as a git tree of `.jjconflict-*` directories, so a git diff that touches one
/// would count those directories rather than the change. jj diffs only these commits. Its diff is
/// single-threaded and, on a 12,000-commit history, roughly ten times slower than the parallel gix
/// diff. A merge gets no counts either, because its numstat is zero whatever jj reports.
const JJ_LOG_TEMPLATE: &str = r#"
commit_id ++ if(
  (conflict || parents.any(|p| p.conflict())) && parents.len() <= 1,
  " " ++ self.diff().stat().files().len()
    ++ " " ++ self.diff().stat().total_added()
    ++ " " ++ self.diff().stat().total_removed()
) ++ "\n"
"#;

fn walk_jj(
    repo: &gix::Repository,
    workspace: &Path,
    revset: &str,
    need_reviewers: bool,
) -> Result<Vec<WalkedCommit>> {
    // Like any jj command, `jj log` snapshots the working copy first, so `@` includes edits that
    // are not yet committed elsewhere.
    let out = jj(
        workspace,
        "log",
        &[
            "log",
            "--no-graph",
            "--revisions",
            revset,
            "--template",
            JJ_LOG_TEMPLATE,
        ],
    )?;
    let commits = out
        .lines()
        .filter_map(|line| match parse_jj_log_line(line) {
            // jj's `root()` is a virtual commit with an all-zero ID and no git object to read.
            Some((id, _)) if id.is_null() => None,
            Some(commit) => Some(Ok(commit)),
            None => Some(Err(Error::JjOutput {
                command: "log",
                output: line.to_string(),
            })),
        });
    read_commits(repo, commits, need_reviewers)
}

/// Parse one line printed by [`JJ_LOG_TEMPLATE`], without its newline.
fn parse_jj_log_line(line: &str) -> Option<(gix::ObjectId, Option<DiffStat>)> {
    let mut fields = line.split(' ');
    let id = gix::ObjectId::from_hex(fields.next()?.as_bytes()).ok()?;
    let diff = match [fields.next(), fields.next(), fields.next(), fields.next()] {
        [None, ..] => None,
        [Some(files), Some(insertions), Some(deletions), None] => Some(DiffStat {
            files: files.parse().ok()?,
            insertions: insertions.parse().ok()?,
            deletions: deletions.parse().ok()?,
        }),
        _ => return None,
    };
    Some((id, diff))
}

/// Read the metadata of each commit in `commits`, keeping their order. Each comes with its numstat
/// when that is already known.
fn read_commits(
    repo: &gix::Repository,
    commits: impl Iterator<Item = Result<(gix::ObjectId, Option<DiffStat>)>>,
    need_reviewers: bool,
) -> Result<Vec<WalkedCommit>> {
    let mailmap = repo.open_mailmap();
    // git grafts commits at the shallow boundary as parentless, so one whose header names several
    // parents is not a merge there. An unreadable shallow file degrades to "not shallow", mirroring
    // Worker::new.
    let shallow = repo.shallow_commits().ok().flatten();
    commits
        .map(|item| {
            let (id, known) = item?;
            let commit = repo
                .find_commit(id)
                .map_err(|e| Error::ReadCommit(Box::new(e)))?;
            // Decode the header once; the author, committer, parents, and message accessors on
            // `gix::Commit` would each rescan the raw bytes.
            let commit = commit.decode().map_err(|e| Error::DecodeCommit {
                id: id.to_string(),
                source: e.into(),
            })?;
            let is_boundary = shallow
                .as_ref()
                .is_some_and(|s| s.binary_search(&id).is_ok());
            let is_merge = !is_boundary && commit.parents.len() > 1;
            Ok(WalkedCommit {
                meta: commit_meta(&commit, &mailmap, need_reviewers)?,
                is_merge,
                handle: known.map_or(Handle::Diff(id), Handle::Known),
            })
        })
        .collect()
}

fn commit_meta(
    commit: &gix::objs::CommitRef<'_>,
    mailmap: &gix::mailmap::Snapshot,
    need_reviewers: bool,
) -> Result<CommitMeta> {
    let author = mailmap.resolve(commit.author().map_err(|e| Error::ReadCommit(e.into()))?);
    let time_seconds = commit
        .committer()
        .map_err(|e| Error::ReadCommit(e.into()))?
        .seconds();
    let reviewers = if need_reviewers {
        parse_reviewers(commit, mailmap)
    } else {
        Vec::new()
    };
    Ok(CommitMeta {
        author: Contributor {
            name: author.name.to_string(),
            email: author.email.to_string(),
        },
        time_seconds,
        reviewers,
    })
}

/// Trailer tokens that name a Reviewer, compared without regard to ASCII case as git compares them.
const REVIEW_TOKENS: [&str; 3] = ["Acked-by", "Tested-by", "Reviewed-by"];

/// The Reviewers that a commit's trailers name, one per review trailer, in message order. Other
/// trailers, such as `Signed-off-by`, are skipped before any parsing or mailmap lookup.
fn parse_reviewers(
    commit: &gix::objs::CommitRef<'_>,
    mailmap: &gix::mailmap::Snapshot,
) -> Vec<Contributor> {
    let Some(body) = commit.message().body() else {
        return Vec::new();
    };
    body.trailers()
        .filter(|t| {
            REVIEW_TOKENS
                .iter()
                .any(|k| t.token.eq_ignore_ascii_case(k.as_bytes()))
        })
        .map(|t| trailer_contributor(&t.value.to_string(), mailmap))
        .collect()
}

/// The Contributor a trailer value names, resolved through `.mailmap` as `git shortlog
/// --group=trailer` does. A value that is not `Name <email>` is taken literally as the name, with
/// no email and no mailmap lookup, as git takes it.
fn trailer_contributor(value: &str, mailmap: &gix::mailmap::Snapshot) -> Contributor {
    let Some((name, email)) = split_ident(value) else {
        return Contributor {
            name: value.to_string(),
            email: String::new(),
        };
    };
    // The mailmap matches on name and email only, so the signature needs no time.
    let resolved = mailmap.resolve_cow(gix::actor::SignatureRef {
        name: name.into(),
        email: email.into(),
        time: "",
    });
    Contributor {
        name: resolved.name.to_string(),
        email: resolved.email.to_string(),
    }
}

/// Split `Name <email>` as git's `split_ident_line` does. The name is everything before the first
/// `<`, without trailing whitespace. The email is everything between that `<` and the first `>`
/// after it, untrimmed. Anything after that `>` is ignored. Returns `None` without a `<`, or
/// without a `>` after it.
fn split_ident(value: &str) -> Option<(&str, &str)> {
    // git's `isspace` is locale-independent and covers only these four characters, so a trailing
    // no-break space or vertical tab stays part of the name.
    const GIT_SPACE: [char; 4] = [' ', '\t', '\n', '\r'];
    let (name, rest) = value.split_once('<')?;
    let (email, _) = rest.split_once('>')?;
    Some((name.trim_end_matches(GIT_SPACE), email))
}

fn numstat_real(
    repo: &gix::Repository,
    walk_cache: &mut gix::diff::blob::Platform,
    count_cache: &mut gix::diff::blob::Platform,
    shallow: Option<&gix::shallow::Commits>,
    id: gix::ObjectId,
) -> Result<DiffStat> {
    let commit = repo
        .find_commit(id)
        .map_err(|e| Error::ReadCommit(Box::new(e)))?;
    let new_tree = commit.tree().map_err(|e| Error::ReadCommit(Box::new(e)))?;
    // A commit at the shallow boundary names parents that are not in the object database. git diffs
    // it against the empty tree, exactly like a root commit, so do the same instead of failing on
    // the missing parent.
    let is_boundary = shallow.is_some_and(|s| s.binary_search(&id).is_ok());
    let parent = if is_boundary {
        None
    } else {
        commit.parent_ids().next()
    };
    let old_tree = match parent {
        Some(parent) => repo
            .find_commit(parent.detach())
            .map_err(|e| Error::ReadCommit(Box::new(e)))?
            .tree()
            .map_err(|e| Error::ReadCommit(Box::new(e)))?,
        None => repo.empty_tree(),
    };

    // Equivalent to `gix::Tree::stats`, but driving caller-owned caches so the index and attribute
    // stack are read once per worker rather than per commit.
    let (mut files, mut insertions, mut deletions) = (0u64, 0u64, 0u64);
    old_tree
        .changes()
        .map_err(|e| Error::DiffStats(Box::new(e)))?
        .for_each_to_obtain_tree_with_cache(&new_tree, walk_cache, |change| {
            if let Some((ins, del)) = gitlink_lines(&change) {
                files += 1;
                insertions += ins;
                deletions += del;
            } else {
                match change
                    .diff(count_cache)
                    .ok()
                    .and_then(|mut platform| platform.line_counts().ok())
                {
                    Some(Some(counts)) => {
                        files += 1;
                        insertions += u64::from(counts.insertions);
                        deletions += u64::from(counts.removals);
                    }
                    // A binary change has no line counts (numstat's `-  -  path`) but still counts
                    // as a changed file, as in `git diff --shortstat`.
                    Some(None) => files += 1,
                    None => {}
                }
            }
            // The resource cache only grows; clear it between changes to bound memory.
            count_cache.clear_resource_cache_keep_allocation();
            Ok(std::ops::ControlFlow::Continue(()))
        })
        .map_err(|e| Error::DiffStats(Box::new(e)))?;
    walk_cache.clear_resource_cache_keep_allocation();

    Ok(DiffStat {
        insertions,
        deletions,
        files,
    })
}

/// The numstat of a submodule (gitlink) change, which carries no blob to diff. git renders the
/// pointer as a one-line `Subproject commit <hash>` pseudo-file, so an added gitlink is +1, a
/// removed one -1, and a repointed one +1/-1. A renamed gitlink is paired into a single file whose
/// line only changes when the commit it points to also changed, matching git's `0 0 old => new`.
/// Returns `None` for changes not purely between gitlinks; a blob<->gitlink type change falls
/// through to the regular blob diff.
fn gitlink_lines(change: &gix::object::tree::diff::Change<'_, '_, '_>) -> Option<(u64, u64)> {
    use gix::object::tree::diff::Change;
    match *change {
        Change::Addition { entry_mode, .. } if entry_mode.is_commit() => Some((1, 0)),
        Change::Deletion { entry_mode, .. } if entry_mode.is_commit() => Some((0, 1)),
        Change::Modification {
            previous_entry_mode,
            entry_mode,
            ..
        } if previous_entry_mode.is_commit() && entry_mode.is_commit() => Some((1, 1)),
        Change::Rewrite {
            source_entry_mode,
            source_id,
            entry_mode,
            id,
            ..
        } if source_entry_mode.is_commit() && entry_mode.is_commit() => {
            if source_id.detach() == id.detach() {
                Some((0, 0))
            } else {
                Some((1, 1))
            }
        }
        _ => None,
    }
}

/// Tips to walk from, and commits to hide, for a revision range.
type RangeEnds = (Vec<gix::ObjectId>, Vec<gix::ObjectId>);

/// Resolve a revision range into walk tips and hidden commits. Each endpoint is fully git-spelled
/// (refs, short hashes, `@{n}`, ...); only the `..`/`...` operators are interpreted here. Exotic
/// gitrevisions(7) forms are unsupported.
fn resolve_range(repo: &gix::Repository, range: &str) -> Result<RangeEnds> {
    if let Some((a, b)) = range.split_once("...") {
        let a = single(repo, default_head(a))?;
        let b = single(repo, default_head(b))?;
        let hidden = merge_bases(repo, a, b, range)?;
        return Ok((vec![a, b], hidden));
    }
    if let Some((a, b)) = range.split_once("..") {
        let excluded = single(repo, default_head(a))?;
        let included = single(repo, default_head(b))?;
        return Ok((vec![included], vec![excluded]));
    }
    Ok((vec![single(repo, range)?], Vec::new()))
}

fn default_head(rev: &str) -> &str {
    if rev.is_empty() { "HEAD" } else { rev }
}

fn single(repo: &gix::Repository, rev: &str) -> Result<gix::ObjectId> {
    let err = |source: Box<dyn std::error::Error + Send + Sync>| Error::ResolveRevision {
        revision: rev.to_string(),
        source,
    };
    // git peels tags recursively at rev-list endpoints, so an annotated tag must resolve to its
    // target commit here. Without peeling, a tag OID on the hidden side matches nothing during the
    // walk and the range silently degrades to whole-repo history.
    Ok(repo
        .rev_parse_single(rev)
        .map_err(|e| err(Box::new(e)))?
        .object()
        .map_err(|e| err(Box::new(e)))?
        .peel_to_commit()
        .map_err(|e| err(Box::new(e)))?
        .id)
}

/// Every merge base of `a` and `b`, hidden from the walk to form the symmetric difference.
/// Criss-cross histories have several, and git hides them all; hiding only the "best" one leaks the
/// other bases' ancestries into the walk. Disjoint histories have none; then nothing is hidden and
/// the symmetric difference is simply everything reachable from either tip.
fn merge_bases(
    repo: &gix::Repository,
    a: gix::ObjectId,
    b: gix::ObjectId,
    range: &str,
) -> Result<Vec<gix::ObjectId>> {
    let bases = repo
        .merge_bases_many(a, &[b])
        .map_err(|e| Error::WalkRange {
            range: range.to_string(),
            source: Box::new(e),
        })?;
    Ok(bases.into_iter().map(gix::Id::detach).collect())
}

/// Parse a `--since`/`--until` style date into seconds since the Unix epoch. Accepts ISO 8601, RFC
/// 2822, unix timestamps, and relative spellings like "2 weeks ago" (a documented subset of git's
/// approxidate).
///
/// # Errors
///
/// Returns an error if the input is not a recognized date format.
pub fn parse_date(input: Option<&str>) -> Result<Option<i64>> {
    let Some(s) = input else { return Ok(None) };
    let now = gix::date::Zoned::now();
    let time = gix::date::parse(s, Some(now)).map_err(|e| Error::InvalidDate {
        input: s.to_string(),
        message: e.to_string(),
    })?;
    Ok(Some(time.seconds))
}

#[cfg(test)]
mod tests {
    use hegel::generators;

    use super::{
        find_jj_workspace, parse_date, parse_jj_log_line, parse_reviewers, split_ident,
        trailer_contributor,
    };
    use crate::model::{Contributor, DiffStat};

    #[test]
    fn parse_date_returns_none_for_no_input() {
        assert_eq!(parse_date(None).unwrap(), None);
    }

    #[test]
    fn parse_date_accepts_an_iso_date() {
        assert!(parse_date(Some("2020-01-01")).unwrap().is_some());
    }

    #[test]
    fn parse_date_rejects_unparseable_input() {
        assert!(parse_date(Some("not-a-real-date")).is_err());
    }

    /// Every line `JJ_LOG_TEMPLATE` can print parses back to what it encodes: a bare commit ID, or
    /// an ID followed by files, insertions, and deletions.
    #[hegel::test]
    fn jj_log_lines_round_trip(tc: hegel::TestCase) {
        let id = gix::ObjectId::Sha1(tc.draw(generators::arrays(generators::integers::<u8>())));
        let diff = tc
            .draw(generators::optional(generators::arrays::<_, u64, 3>(
                generators::integers(),
            )))
            .map(|[files, insertions, deletions]| DiffStat {
                insertions,
                deletions,
                files,
            });
        let line = match diff {
            None => id.to_string(),
            Some(d) => format!("{id} {} {} {}", d.files, d.insertions, d.deletions),
        };
        assert_eq!(parse_jj_log_line(&line), Some((id, diff)));
    }

    /// A line with a count field missing or extra is malformed, never a shifted or truncated
    /// numstat.
    #[hegel::test]
    fn jj_log_lines_reject_wrong_field_counts(tc: hegel::TestCase) {
        let id = gix::ObjectId::Sha1(tc.draw(generators::arrays(generators::integers::<u8>())));
        let counts = tc.draw(generators::vecs(generators::integers::<u64>()).max_size(8));
        tc.assume(!counts.is_empty() && counts.len() != 3);
        let line = std::iter::once(id.to_string())
            .chain(counts.iter().map(u64::to_string))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(parse_jj_log_line(&line), None);
    }

    /// The workspace is the nearest directory holding `.jj`. When stopping at git, a nearer `.git`
    /// (directory or file) means there is none, while a directory holding both, as a colocated
    /// workspace does, is still jj.
    // Each case creates up to 15 directories and files. 50 cases cover the 6 distinct marker sets
    // per level while keeping the test under a second.
    #[hegel::test(test_cases = 50)]
    fn jj_workspace_is_the_nearest_jj_directory(tc: hegel::TestCase) {
        // One marker set per nested directory, outermost first. Bit 0 adds a `.jj` directory, bit 1
        // a `.git`, and bit 2 makes that `.git` a file.
        let levels = tc.draw(
            generators::vecs(generators::integers::<u8>().max_value(7))
                .min_size(1)
                .max_size(5),
        );
        let stop_at_git = tc.draw(generators::booleans());

        let root = tempfile::tempdir().unwrap();
        let mut dir = root.path().to_path_buf();
        let mut dirs = Vec::new();
        for (i, &markers) in levels.iter().enumerate() {
            dir.push(format!("l{i}"));
            std::fs::create_dir(&dir).unwrap();
            if markers & 1 != 0 {
                std::fs::create_dir(dir.join(".jj")).unwrap();
            }
            if markers & 2 != 0 {
                if markers & 4 != 0 {
                    std::fs::write(dir.join(".git"), "gitdir: elsewhere\n").unwrap();
                } else {
                    std::fs::create_dir(dir.join(".git")).unwrap();
                }
            }
            dirs.push(dir.canonicalize().unwrap());
        }

        // `Some` when one of our directories decides the answer; `None` when the search must
        // continue above the temporary directory.
        let mut decided = None;
        for (markers, dir) in levels.iter().zip(&dirs).rev() {
            if markers & 1 != 0 {
                decided = Some(Some(dir.clone()));
                break;
            }
            if stop_at_git && markers & 2 != 0 {
                decided = Some(None);
                break;
            }
        }
        let found = find_jj_workspace(&dir, stop_at_git);
        match decided {
            Some(expected) => assert_eq!(found, expected),
            // The host's own directories may hold a workspace; it just must not be one of ours.
            None => assert!(
                found.as_ref().is_none_or(|f| !f.starts_with(&dirs[0])),
                "found {found:?}"
            ),
        }
    }

    /// Each case is how git's `split_ident_line` splits the value, so the Reviewer row matches `git
    /// shortlog --group=trailer`.
    #[test]
    fn split_ident_follows_git() {
        let cases = [
            ("Ada Lovelace <ada@x>", Some(("Ada Lovelace", "ada@x"))),
            ("<ada@x>", Some(("", "ada@x"))),
            ("Ada  \t<ada@x>", Some(("Ada", "ada@x"))),
            ("Ada<ada@x>", Some(("Ada", "ada@x"))),
            ("Ada < ada@x >", Some(("Ada", " ada@x "))),
            ("Ada <ada@x> # see <foo>", Some(("Ada", "ada@x"))),
            ("Ada <>", Some(("Ada", ""))),
            ("Ada\r\n <ada@x>", Some(("Ada", "ada@x"))),
            ("Ada\u{a0} <ada@x>", Some(("Ada\u{a0}", "ada@x"))),
            ("Ada\u{b} <ada@x>", Some(("Ada\u{b}", "ada@x"))),
            ("Ada", None),
            ("Ada <ada@x", None),
            ("Ada ada@x>", None),
        ];
        for (value, want) in cases {
            assert_eq!(split_ident(value), want, "value: {value:?}");
        }
    }

    /// Any `Name <email>` splits back into its parts, whatever follows the `>`.
    #[hegel::test]
    fn split_ident_recovers_name_and_email(tc: hegel::TestCase) {
        let name = tc.draw(generators::from_regex(r"([^<\s]([^<]*[^<\s])?)?").fullmatch(true));
        let email = tc.draw(generators::from_regex(r"[^>]*").fullmatch(true));
        let gap = tc.draw(generators::from_regex(r"[ \t]*").fullmatch(true));
        let tail = tc.draw(generators::text());
        let value = format!("{name}{gap}<{email}>{tail}");
        assert_eq!(split_ident(&value), Some((name.as_str(), email.as_str())));
    }

    #[test]
    fn trailer_contributors_resolve_through_the_mailmap() {
        let mailmap = gix::mailmap::Snapshot::from_bytes(b"New Name <new@x> <old@x>\n");
        let who = |name: &str, email: &str| Contributor {
            name: name.to_string(),
            email: email.to_string(),
        };
        assert_eq!(
            trailer_contributor("Old Name <old@x>", &mailmap),
            who("New Name", "new@x")
        );
        assert_eq!(
            trailer_contributor("Other <other@x>", &mailmap),
            who("Other", "other@x")
        );
        // git takes a value that is not an identity literally, without a mailmap lookup.
        assert_eq!(trailer_contributor("old@x", &mailmap), who("old@x", ""));
    }

    /// Only review trailers name Reviewers, whatever their case, one entry per trailer in message
    /// order.
    #[test]
    fn parse_reviewers_keeps_only_review_trailers() {
        let raw = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
            author A <a@x> 1600000000 +0000\n\
            committer A <a@x> 1600000000 +0000\n\
            \n\
            subject\n\
            \n\
            body\n\
            \n\
            Signed-off-by: A <a@x>\n\
            reviewed-BY: Rev <rev@x>\n\
            Tested-by: Rev <rev@x>\n\
            Acked-by: Ack <ack@x>\n";
        let commit = gix::objs::CommitRef::from_bytes(raw, gix::hash::Kind::Sha1).unwrap();
        let names: Vec<String> = parse_reviewers(&commit, &gix::mailmap::Snapshot::default())
            .into_iter()
            .map(|c| c.ident())
            .collect();
        assert_eq!(names, ["Rev <rev@x>", "Rev <rev@x>", "Ack <ack@x>"]);
    }
}
