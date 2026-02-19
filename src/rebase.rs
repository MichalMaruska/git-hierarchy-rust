#![deny(elided_lifetimes_in_paths)]

// rebase segment.
use git2::{Branch, BranchType, Error, Commit,
           Oid,
           Reference, Repository,RepositoryState,
           StatusOptions, StatusShow,

           CherrypickOptions,
           MergeOptions,
           build::CheckoutBuilder,
};

use crate::utils::{iterator_symmetric_difference};

#[allow(unused)]
use crate::git_hierarchy::{GitHierarchy, Segment, Sum, load};
use crate::graph::discover::NodeExpander;

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
use colored::Colorize;


pub enum RebaseResult {
    Nothing,
    Done,
    // Failed,
}

#[non_exhaustive]
#[derive(Debug)]
pub enum RebaseError {
    WrongHierarchy(String),
    WrongState,
    Default,
}

impl std::convert::From<crate::execute::Error> for RebaseError {
    fn from(_e: crate::execute::Error) -> RebaseError{
       RebaseError::Default
    }
}

impl std::convert::From<git2::Error> for RebaseError {
    fn from(_e: git2::Error) -> RebaseError{
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
/// see `segment_to_continue'() for the read part
fn record_processed_commit(repository: &'_ Repository, oid: Oid, applied: bool) -> io::Result<()>{
    let path = marker_filename(repository);
    debug!("Update persistent state: {:?}", path);

    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap();

    let marker =
        if applied {
            "0"
        } else {
            "1"
        };
    writeln!(file, "{}", marker)?;
    writeln!(file, "{}", &format!("{}", oid))?;
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
        eprintln!("{}",Colorize::red("SORRY conflicts detected"));
        eprintln!("{}",Colorize::red("resolve them, and either commit or stage them"));

        // next time resume from this, `exclusive'.
        record_processed_commit(repository, original.id(), false).unwrap();
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
// cherry-picks each commits from the iterator, and returns the HEAD afterwards/on error?
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
                          record_processed_commit(repository, to_apply.id(), true).unwrap();

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
        return rebase_empty_segment(segment, repository);
    }

    // fixme: if we are in the middle of rebase?
    if repository.state() != RepositoryState::Clean {
        debug!("the repository is not clean");
        return Err(RebaseError::WrongState);
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
            debug!("git cherry-pick failed");
            return Err(RebaseError::Default)
        } else {
            return Ok(RebaseResult::Done);
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
        info!("git cherry-pick --continue failed");
        return Err(RebaseError::Default);
    }

    if let GitHierarchy::Segment(segment) = load(repository, segment_name)? {
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
            Err(RebaseError::Default)
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
                                       skip: usize
) -> Result<(), RebaseError> {
    // Find & skip:
    let iter = segment.iter(repository).unwrap()
        .skip_while(|x| x.as_ref().unwrap() != &commit_id );

    let mut peek = iter.peekable();
    if peek.peek().is_none() {
        debug!("Couldn't find the commmit on the segment");
        return Err(RebaseError::WrongHierarchy(segment.name().to_owned()));
    }

    // todo: check the index
    let parent = repository.head()?.peel_to_commit()?;

    // here we continue the whole sub-segment chain:
    debug!("now continue to pick the rest of the segment '{}'", segment.name());

    // check we are in a clean state!
    // The default, if unspecified, is to show the index and the working
    let statuses = repository.statuses(None)?;
    if ! statuses.len() == 0 {
        eprintln!("Status is not clean!");
        return Err(RebaseError::WrongState);
    }

    let commit = cherry_pick_commits(repository,
                                     peek.skip(skip),
                                     parent).unwrap();
    // might need this if nothing to cherrypick anymore.
    segment.reset(repository, commit.id());
    // fixme:
    // rebase_segment_finish(
    Ok(())
}


// loads the persistent state.
fn segment_to_continue(repository: &Repository) -> Result<(String,String,usize), RebaseError>
{
    let path = marker_filename(repository);

    if ! fs::exists(&path).unwrap() {
        error!("marker file does not exist -- no segment is being rebased.");
        return Err(RebaseError::WrongState);
    }

    let content: String = fs::read_to_string(path).unwrap(); // .... and the oid
    let mut lines = content.lines();
    let segment_name = lines.next().unwrap().trim();

    let oid = lines.next_back().unwrap();
    let skip : usize = lines.next_back().unwrap().parse().unwrap();

    debug!("from file: continue on {}, after {:?}", segment_name, oid);
    Ok((segment_name.to_owned(), oid.to_owned(), skip))
}

// Continue after an issue:
// either cherry-pick conflicts resolved by the user, or
// he left mess, and ....on detached head. Unlike other tools.
pub fn rebase_segment_continue(repository: &Repository) -> Result<RebaseResult, RebaseError> {

    let (segment_name,oid,mut skip) = segment_to_continue(repository)?;

    if let GitHierarchy::Segment(segment) = load(repository, &segment_name).unwrap() {
        let commit_id =
            if repository.state() == RepositoryState::CherryPick {
                // read the CHERRY_PICK_HEAD
                // todo: convert to step.step2...
                let commit_id = Oid::from_str(read_cherry_pick_head(repository).as_str().trim()).unwrap();
                debug!("should continue the cherry-pick {:?}", commit_id);

                let mut option =  StatusOptions::new();
                option.show(StatusShow::Index);
                let statuses = repository.statuses(Some(&mut option))?;
                if ! statuses.is_empty() {
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
                Oid::from_str(&oid).unwrap()
            };

        eprintln!("should cherry-pick starting from oid {}", commit_id);

        assert!(repository_clean(repository));
        continue_segment_cherry_pick(repository, &segment, commit_id, skip)?;

        segment.reset(repository,
                      repository.head().unwrap().peel_to_commit().unwrap().id());

        let tmp_head: Branch<'_> = repository
            .find_branch(TEMP_HEAD_NAME, BranchType::Local)
            .unwrap();
        cleanup_segment_rebase(repository, &segment, tmp_head);
        Ok(RebaseResult::Done)
    } else {
        Err(RebaseError::WrongHierarchy(segment_name))
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
) -> Result<RebaseResult, RebaseError> {
    debug!("rebase empty segment: {}", segment.name());

    segment.reset(repository,
                  segment.base(repository).peel_to_commit()?.id());
    Ok(RebaseResult::Done)
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

pub fn check_segment(repository: &Repository, segment: &Segment<'_>) -> Result<(), RebaseError>
{
    // no merge commits
    if ! is_linear_ancestor(repository,
                            segment.start(),
                            segment.reference.borrow().target().unwrap())? {
        warn!("check_segment failed for {}", segment.name());
        return Err(RebaseError::WrongHierarchy(segment.name().to_owned()));
    }

    // no segments inside. lenght limited....

    // git_revisions()
    // walk.push_ref(segment.reference.borrow());
    // walk.hide(segment._start.target().unwrap());
    // walk.hide_ref(ref);

    // push_range
    // descendant of start.

    // start.is_ancestor(reference);
    Ok(())
}

pub fn check_sum<'repo>(
    repository: &'repo Repository,
    sum: &Sum<'repo>,
    object_map: &HashMap<String, GitHierarchy<'repo>>,
) -> Result<(), RebaseError> {
    let count = sum.reference.borrow().peel_to_commit().unwrap().parent_count();

    // terrible:
    // !i>2 in Rust  means ~i>2 in C
    // https://users.rust-lang.org/t/why-does-rust-use-the-same-symbol-for-bitwise-not-or-inverse-and-logical-negation/117337/2
    if count <= 1 {
        warn!("not a merge: {}, only {} parent commits", sum.name(), count);
        return Err(RebaseError::WrongHierarchy(sum.name().to_owned()));
    };

    // each of the summands has relationship to a parent commit.
    let summands = sum.summands(repository);
    /* assumption:
    sum has its summands   base/1 ... base/N
    these might resolve to References. -- how is that different from Branch?

    During the rebasing we change ... Branches (References), and update them in the `object_map'
    so we .... prefer to look up there.
     */

    // find the representation which we already have and keep updating.

    // Map through object_map to the Nodes:
    let graphed_summands: Vec<&GitHierarchy<'_>> = summands
        .iter()
        .map(
            |s| {
                let gh = object_map.get(s.name().unwrap()).unwrap();
                debug!(
                    "convert {:?} to {:?}",
                    s.name().unwrap(),
                    gh.node_identity()
                );
                gh
            })
        .collect();

    let parent_commits = sum.parent_commits();

    debug!("The current parent commits are: {:?}", parent_commits);
    for c in sum.parent_commits() {
        debug!("  {}", c);
    }

    let (u,v) = iterator_symmetric_difference(
        graphed_summands.iter().map(|gh| {
            debug!("mapping {:?} to {:?}", gh.node_identity(),
                   gh.commit().unwrap().id());
            gh.commit().unwrap().id()
        }),
        parent_commits);


    if !(u.is_empty() && v.is_empty()) {
        warn!("sum {} is not well-positioned", sum.name());
        return Err(RebaseError::WrongHierarchy(sum.name().to_owned()));
    }
    /*
    (mapped, rest_summands, left_overs_parent_commits) = distribute(sum);
    // either it went ahead ....or? what if it's rebased?
    for bad in rest_summands {
        // try to find in over
        find_ancestor()
    }

    */

    Ok(())
}


