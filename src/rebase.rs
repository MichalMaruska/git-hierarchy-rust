#![deny(elided_lifetimes_in_paths)]

// rebase segment.
use git2::{Branch, BranchType, Error, Commit,
           Oid,
           Reference, Repository,RepositoryState,

           CherrypickOptions,
           MergeOptions,
           build::CheckoutBuilder,
};


#[allow(unused)]
use crate::git_hierarchy::{GitHierarchy, Segment, Sum, load};

use crate::execute::git_run;
use crate::base::{checkout_new_head_at,
                  repository_clean,
                  force_head_to,
                  staged_files,
                  is_linear_ancestor,
};

use std::collections::HashMap;
use std::fs::{self,OpenOptions};
use std::io::{Write,self};
use std::path::PathBuf;
use std::process::exit; // fixme: drop in library
#[allow(unused_imports)]
use tracing::{span, Level, debug, info, warn,error};

pub enum RebaseResult {
    Nothing,
    Done,
    // Failed,
}

#[non_exhaustive]
#[derive(Debug)]
pub enum RebaseError {
    Default
}

impl std::convert::From<crate::execute::Error> for RebaseError {
    fn from(_e: crate::execute::Error) -> RebaseError{
       RebaseError::Default
    }
}


const TEMP_HEAD_NAME: &str = "tempSegment";
const MARKER_FILENAME: &str = ".segment-cherry-pick";

fn marker_filename(repository: &Repository) -> PathBuf {
    repository.commondir().join(MARKER_FILENAME)
}

fn create_marker_file(repository: &Repository, content: &str) -> io::Result<()> {
    let path = marker_filename(repository);
    // todo: a Git reference?
    // persistent mark, if we fail, and during the session.
    debug!("Create marker: {:?}", path);
    fs::write(path, content)
}

/// Store persistently (between runs) the commit we stubled on
fn append_oid(repository: &'_ Repository, oid: &str) -> io::Result<()> {
    let path = marker_filename(repository);
    debug!("Update persistent state: {:?}", path);

    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap();

    if let Err(e) = writeln!(file, "{}", oid) {
        eprintln!("Couldn't write to file: {}", e);
        return Err(e);
    }
    Ok(())
}

fn read_cherry_pick_head(repository: &'_ Repository) -> String {
    fs::read_to_string(repository.commondir().join("CHERRY_PICK_HEAD")).unwrap()
}

/// Creates each commit during the rebase/cherry-picking: both in OK flow
/// and after manual intervention.  Can the user do the commit himself? -- do we setup the ....
/// @original is the original commit we try to clone.
///
fn commit_cherry_picked<'repo>(repository: &'repo Repository,
                               original: &Commit<'repo>,
                               parent_commit: &Commit<'repo>) -> Oid {
    let mut index = repository.index().unwrap();
    if index.has_conflicts() {
        eprintln!("SORRY conflicts detected");
        eprintln!("resolve them, and either commit or stage them");

        // next time resumve from this, exclusive.
        // todo: unify these 2 calls into 1
        // record_applied(original.id())
        append_oid(repository, "1").unwrap();
        append_oid(repository, &format!("{}", original.id())).unwrap();
        exit(1);
    }

    let statusses = staged_files(repository).unwrap();
    if statusses.is_empty() {
        eprintln!("SORRY nothing staged, empty -- skip?");
        // so we have .git/CHERRY_PICK_HEAD ?
        exit(1);
    } else {
        info!("something staged");
    }

    let tree_oid = index.write_tree().unwrap();
    let new_oid =
        if repository.head().unwrap().peel_to_tree().unwrap().id() == tree_oid {
            warn!("SORRY nothing staged, empty -- skip?");
            // bug: and no changes in the worktree!
            repository.head().unwrap().target().unwrap()
            // silently skipping over?
            // exit(1);
        } else {
            // same tree id ... it was empty!

            //  "cannot create a tree from a not fully merged index."
            let tree = repository.find_tree(tree_oid).unwrap();

            repository.commit(
                Some("HEAD"),
                // copy over:
                &original.author(),
                &original.committer(),
                original.message().unwrap(),
                // and timestamps? part of those ^^ !
                &tree,
                &[parent_commit],
            ).unwrap()
        };

    repository.cleanup_state().unwrap();
    new_oid
}


// on top of HEAD
fn cherry_pick_commits<'repo, T>(repository: &'repo Repository,
                                 iter: T,
                                 base_commit: Commit<'repo>)
                                 -> Result<Commit<'repo>, Error>
    where T: Iterator<Item = Result<Oid, Error> >
{
    let final_commit =
        iter.fold(base_commit,
                  |base_commit, oid_to_apply| {
                      // todo: these are always the same
                      let mut options = MergeOptions::new();
                      options.fail_on_conflict(true)
                          .standard_style(true)
                          .ignore_whitespace(true)
                          .patience(true)
                          .minimal(true)
                          ;

                      let to_apply = repository.find_commit(oid_to_apply.unwrap()).unwrap();

                      // use `cherrypick'

                      info!("cherry-pick commit: {:?}", to_apply);

                      let mut checkout_opts = CheckoutBuilder::new();
                      checkout_opts.safe();

                      // todo: we need to register that we resume from some point.
                      // if this fails, the user might .... clean up the repo,
                      // and we can resume.
                      let mut cherrypick_opts = CherrypickOptions::new();
                      cherrypick_opts.checkout_builder(checkout_opts);

                      let result = repository.cherrypick(&to_apply, Some(&mut cherrypick_opts));

                      if let Err(e) = result {
                          eprintln!("cherrypick failed on {}\n {:?}",
                                    to_apply.id(), e);
                          eprintln!("error: code{:?}, class {:?}: {}",
                                    e.code(),
                                    e.class(),
                                    e.message()
                          );
                          // code: -13, klass: 22, message: "1 uncommitted change would be overwritten by merge" }
                          append_oid(repository, "0").unwrap();
                          append_oid(repository, &format!("{}", to_apply.id())).unwrap();

                          let index = repository.index().unwrap();
                          if index.has_conflicts() {
                              eprintln!("{}: SORRY conflicts detected", line!());
                          }

                          eprintln!("should skip");
                          exit(1);
                      }

                      let new_oid = commit_cherry_picked(repository, &to_apply, &base_commit);
                      let new_commit = repository.find_commit(new_oid).unwrap();

                      if false {
                          info!("SLEEP");
                          // sleep(Duration::from_secs(2));
                      }

                      // return:
                      new_commit});

    Ok(final_commit)
}

/// Given a @segment, and HEAD ....
/// either exit or rewrite the segment ....its reference should update oid.
pub fn rebase_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>) -> Result<RebaseResult, RebaseError> {
    if segment.uptodate(repository) {
        info!("nothing to do -- base and start equal");
        return Ok(RebaseResult::Nothing);
    }

    let new_start = segment.base(repository);

    if segment.empty(repository) {
        return Ok(rebase_empty_segment(segment, repository));
    }

    // fixme: if we are in the middle of rebase?
    if repository.state() != RepositoryState::Clean {
        panic!("the repository is not clean");
    }

    info!("rebase_segment: {}", segment.name());
    debug!("rebasing by Cherry-picking {}!", segment.name());
    // can I raii ? so drop() would remove the file?
    create_marker_file(repository,
                       &format!("{}\n", segment.name())).unwrap();

    // checkout to that ref
    // todo: git stash
    // must change to the directory!
    let temp_head = TEMP_HEAD_NAME;
    Branch::name_is_valid(temp_head).unwrap();
    let mut temp_head =
        checkout_new_head_at(repository, Some(temp_head),
                             &new_start.peel_to_commit().unwrap())
        .unwrap();

    let sha = new_start.peel_to_commit().unwrap().id();
    debug!("set-head: {:?}", &sha);
    // If I cherry-pick with temp as HEAD, it fails with ... "old reference value does not match"
    repository.set_head_detached(sha).unwrap();
    /*
    repository.set_head_bytes(sha.as_bytes()).unwrap();
    */
    //  "the given reference name 'bdcaa23cbe4ea0b6316caf82a9afb96e7c7f1fe6' is not valid"
    debug!("checkout: {:?}", repository.head().unwrap().name());
    // bug: goes out of sync.
    if false {
        if !git_run(
            repository,
            &["cherry-pick", segment.git_revisions().as_str()],
        )
            .is_ok_and(|x| x.success())
        {
            // return RebaseResult::Failed;
            panic!("cherry-pick failed");
        }
    } else {
        let commit = cherry_pick_commits(repository,
                                         segment.iter(repository).unwrap(),
                                         segment.base(repository).peel_to_commit().unwrap()
                                         ).unwrap();
        // move
        segment.reset(repository, commit.id());

        // mmc: why still keeping the temp_head?
        // set temp_head to point at commit:
        debug!("setting {:?} at {:?} to {:?}",temp_head.name().unwrap().unwrap(),
               temp_head.get().target().unwrap(),
               commit.id());
        temp_head = Branch::wrap(
            temp_head.get_mut().set_target(commit.id(), "rebased").unwrap());
    }

    // I have to re-find it?
    rebase_segment_finish(
        repository,
        segment,
        // temp_head.get()
        repository
            .find_branch(TEMP_HEAD_NAME, BranchType::Local)
            .unwrap()
            .get(),
    );
    cleanup_segment_rebase(repository, segment, temp_head);
    Ok(RebaseResult::Done)
}

// The old, using git(1)
fn rebase_continue_git1(repository: &Repository, segment_name: &str) -> Result<RebaseResult, RebaseError> {
    if !git_run(repository, &["cherry-pick", "--continue"]).is_ok_and(|x| x.success()) {
        info!("Good?")
        // panic!("cherry-pick failed");
    }

    if let GitHierarchy::Segment(segment) = load(repository, segment_name).unwrap() {
        let tmp_head: Branch<'_> = repository
            .find_branch(TEMP_HEAD_NAME, BranchType::Local)
            .unwrap();
        if tmp_head.is_head() {
            //name: &str, branch_type: BranchType) -> Result<Branch<'_>, Error> {head();
            rebase_segment_finish(
                repository,
                &segment,
                repository
                    .find_branch(TEMP_HEAD_NAME, BranchType::Local)
                    .unwrap()
                    .get(),
            );
            cleanup_segment_rebase(repository, &segment, tmp_head);
            Ok(RebaseResult::Done)
        } else {
            // mismatch
            panic!();
        }
    } else {
        Ok(RebaseResult::Nothing)
    }
}

/// resume rebasing segment, from certain commit, exclusive/inclusive based on `skip'.
// HEAD is already correct.
// can the status be still CHERRY_PICK ?
fn continue_segment_cherry_pick<'repo>(repository: &'repo Repository,
                                       segment: &'_ Segment<'repo>,
                                       commit_id: Oid,
                                       skip: usize) {
    // Find & skip:
    let iter = segment.iter(repository).unwrap()
        .skip_while(|x| x.as_ref().unwrap() != &commit_id );

    let mut peek = iter.peekable();
    if peek.peek().is_none() {
        println!("Empty!");
        panic!("not found or last");
    }

    // todo: check the index
    let parent = repository.head().unwrap().peel_to_commit().unwrap();

    // panic!("not supported currently");
    // commit
    // find where to resume

    // here we continue the whole sub-segment chain:
    debug!("now continue to pick the rest of the segment '{}'", segment.name());

    // check we are in a clean state!
    // The default, if unspecified, is to show the index and the working
    let statuses = repository.statuses(None).unwrap();
    if ! statuses.len() == 0 {
        eprintln!("Status is not clean!");
        exit(-1);
    }

    let commit = cherry_pick_commits(repository,
                                     peek.skip(skip),
                                     parent).unwrap();
    // might need this if nothing to cherrypick anymore.
    segment.reset(repository, commit.id());
    // fixme:
    // rebase_segment_finish(
}


// Continue after an issue:
// either cherry-pick conflicts resolved by the user, or
// he left mess, and ....on detached head. Unlike other tools.
pub fn rebase_segment_continue(repository: &Repository) -> Result<RebaseResult, RebaseError> {
    let path = marker_filename(repository);

    // todo: maybe this before calling this function?
    if ! fs::exists(&path).unwrap() {
        error!("marker file does not exist -- no segment is being rebased.");
        exit(1);
    }

    // load persistent state:
    let content: String = fs::read_to_string(path).unwrap(); // .... and the oid
    let mut lines = content.lines();
    let segment_name = lines.next().unwrap().trim();

    if false {
        rebase_continue_git1(repository, segment_name)
    } else if let GitHierarchy::Segment(segment) = load(repository, segment_name).unwrap() {
        // higher level .. our file:

        // this should contain the `skip'
        let oid = lines.next_back().unwrap();
        let mut skip : usize = lines.next_back().unwrap().parse().unwrap();
        // native:
        debug!("from file: continue on {}, after {:?}", segment_name, oid);

        let commit_id =
            if repository.state() == RepositoryState::CherryPick {
                // read the CHERRY_PICK_HEAD
                // todo: convert to step.step2...
                let commit_id = Oid::from_str(read_cherry_pick_head(repository).as_str().trim()).unwrap();
                debug!("should continue the cherry-pick {:?}", commit_id);

                // commit it, or reset the state?
                if !repository.index().unwrap().is_empty() {
                    debug!("non-empty index -> commit...");
                    let to_apply = repository.find_commit(commit_id).unwrap();

                    let parent = repository.head().unwrap().peel_to_commit().unwrap();
                    let new_oid = commit_cherry_picked(repository,
                                                       &to_apply,
                                                       &parent);
                    debug!("new commit created {new_oid}");
                    // parent = repository.find_commit(new_oid).unwrap();
                } else {
                    // the user might have decided to drop this change -- skip over.
                    // todo: reset
                    info!("Cleaning cherry pick info: user unstaged the change");
                    repository.cleanup_state().unwrap();
                }
                skip = 1;
                // we need the next one.
                commit_id
            } else {
                Oid::from_str(oid).unwrap()
            };

        eprintln!("should cherry-pick starting from oid {}", commit_id);

        assert!(repository_clean(repository));
        continue_segment_cherry_pick(repository, &segment, commit_id, skip);

        segment.reset(repository,
                      repository.head().unwrap().peel_to_commit().unwrap().id());

        let tmp_head: Branch<'_> = repository
            .find_branch(TEMP_HEAD_NAME, BranchType::Local)
            .unwrap();
        cleanup_segment_rebase(repository, &segment, tmp_head);
        Ok(RebaseResult::Done)
    } else {
        panic!("segment not found");
    }
}

// strange this mut. caller makes a copy?
fn drop_temporary_head(repository: &Repository, mut temp_head: Branch<'_>) {
    if true {
        temp_head.delete().expect("failed to delete a branch");
    } else if !git_run(
            repository,
            &["branch", "-D", temp_head.name().unwrap().unwrap()],
        ).is_ok_and(|x| x.success())
        {
            panic!("branch -D failed");
        }
}

// bad name:
fn cleanup_segment_rebase(repository: &Repository, _segment: &Segment<'_>,
                          temp_head: Branch<'_>) {

    debug!("delete: {:?} {}", temp_head.name().unwrap().unwrap(),
           temp_head.get().target().unwrap());

    drop_temporary_head(repository, temp_head);

    let path = marker_filename(repository);
    debug!("delete: {:?}", path);
    fs::remove_file(path).unwrap();
}

fn rebase_empty_segment<'repo>(
    segment: &Segment<'repo>,
    repository: &'repo Repository,
) -> RebaseResult {
    debug!("rebase empty segment: {}", segment.name());

    segment.reset(repository,
                  segment.base(repository).peel_to_commit().unwrap().id());
    RebaseResult::Done
}

fn rebase_segment_finish<'repo>(
    repository: &'repo Repository,
    segment: &Segment<'repo>,
    new_head: &Reference<'_>,
) {
    // bug: segment.reset(repository);  // bug: does not reload the head of the segment!

    // reflog etc.
    force_head_to(repository, segment.name(), new_head);
}

pub fn check_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>)
{
    // no merge commits
    if ! is_linear_ancestor(repository,
                            segment.start(),
                            segment.reference.borrow().target().unwrap()) {
        panic!("segment {} in mess", segment.name());
    }

    // no segments inside. lenght limited....

    // git_revisions()
    // walk.push_ref(segment.reference.borrow());
    // walk.hide(segment._start.target().unwrap());
    // walk.hide_ref(ref);

    // push_range
    // descendant of start.

    // start.is_ancestor(reference);
}

pub fn check_sum<'repo>(
    _repository: &'repo Repository,
    sum: &Sum<'repo>,
    _object_map: &HashMap<String, GitHierarchy<'repo>>,
) {
    let count = sum.reference.borrow().peel_to_commit().unwrap().parent_count();

    // terrible:
    // !i>2 in Rust  means ~i>2 in C
    // https://users.rust-lang.org/t/why-does-rust-use-the-same-symbol-for-bitwise-not-or-inverse-and-logical-negation/117337/2
    if count <= 1 {
        panic!("not a merge: {}, only {} parent commits", sum.name(), count);
    };

    // each of the summands has relationship to a parent commit.
    // divide
    /*
    (mapped, rest_summands, left_overs_parent_commits) = distribute(sum);
    // either it went ahead ....or? what if it's rebased?
    for bad in rest_summands {
        // try to find in over
        find_ancestor()
    }

    */
}


