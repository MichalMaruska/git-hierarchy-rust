#![deny(elided_lifetimes_in_paths)]
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

use clap::Parser;
#[allow(unused_imports)]
use git2::{Branch, BranchType, Error, Commit, Reference, ReferenceFormat, Repository,
           MergeOptions, CherrypickOptions,
           build::CheckoutBuilder,
           Oid,
           RepositoryState,
           // merge:
           AnnotatedCommit,
           Sort,
};

#[allow(unused_imports)]
use tracing::{span, Level, debug, info, warn,error};

use ::git_hierarchy::base::{checkout_new_head_at, git_same_ref};
use ::git_hierarchy::execute::git_run;
use ::git_hierarchy::utils::{
    concatenate, divide_str, extract_name, find_non_matching_elements, init_tracing,
};
use std::collections::HashMap;
use std::iter::Iterator;

use crate::graph::discover_pet::find_hierarchy;

// I need both:
#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy, Segment, Sum, load};

use std::fs::{self,OpenOptions};
use std::io::{Write,self};
use std::path::PathBuf;
use std::process::exit;

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph;
use graph::discover::NodeExpander;

enum RebaseResult {
    Nothing,
    Done,
    // Failed,
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
    return Ok(());
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
        eprintln!("{} SORRY conflicts detected", line!());
        eprintln!("resolve them, and either commit or stage them");

        // next time resumve from this, exclusive.
        append_oid(repository, "1").unwrap();
        append_oid(repository, &format!("{}", original.id())).unwrap();
        exit(1);
    }
    if index.is_empty() {
        // eprintln
        warn!("SORRY nothing staged, empty -- skip?");
        // so we have .git/CHERRY_PICK_HEAD ?
        exit(1);
    } else {
        info!("not empty, something staged, will commit. {}", index.len());
    }

    let tree_oid = index.write_tree().unwrap();

    let new_oid;

    if repository.head().unwrap().peel_to_tree().unwrap().id()
        == tree_oid {
            warn!("SORRY nothing staged, empty -- skip?");
            // bug: and no changes in the worktree!
            new_oid = repository.head().unwrap().target().unwrap();
            // silently skipping over?
            // exit(1);
        } else {
            // same tree id ... it was empty!


            //  "cannot create a tree from a not fully merged index."
            let tree = repository.find_tree(tree_oid).unwrap();
            new_oid = repository.commit(
                Some("HEAD"),
                // copy over:
                &original.author(),
                &original.committer(),
                &original.message().unwrap(),
                // and timestamps? part of those ^^ !
                &tree,
                &[parent_commit],
            ).unwrap();
        }

    repository.cleanup_state().unwrap();
    return new_oid;
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

                      let mut cherrypick_opts = CherrypickOptions::new();
                      cherrypick_opts.checkout_builder(checkout_opts);

                      let result = repository.cherrypick(&to_apply, Some(&mut cherrypick_opts));

                      if !(result.is_ok()) {
                          eprintln!("cherrypick failed {:?}", result.err());
                          let index = repository.index().unwrap();
                          if index.has_conflicts() {
                              eprintln!("SORRY conflicts detected");
                          }
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

    return Ok(final_commit);
}

// either exit or rewrite the segment ....its reference should update oid.
fn rebase_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>) -> RebaseResult {
    if segment.uptodate(repository) {
        info!("nothing to do -- base and start equal");
        return RebaseResult::Nothing;
    }

    let new_start = segment.base(repository);

    // todo: segment_empty()
    if segment.empty(repository) {
        return rebase_empty_segment(segment, repository);
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
            .success()
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
            .find_branch(&TEMP_HEAD_NAME, BranchType::Local)
            .unwrap()
            .get(),
    );
    cleanup_segment_rebase(repository, segment, temp_head);
    return RebaseResult::Done;
}

fn rebase_continue_git1(repository: &Repository, segment_name: &str) -> RebaseResult {
    if !git_run(repository, &["cherry-pick", "--continue"]).success() {
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
                    .find_branch(&TEMP_HEAD_NAME, BranchType::Local)
                    .unwrap()
                    .get(),
            );
            cleanup_segment_rebase(repository, &segment, tmp_head);
            return RebaseResult::Done;
        } else {
            // mismatch
            panic!();
        }
    } else {
        RebaseResult::Nothing
    }
}

fn continue_segment_cherry_pick<'repo, 'a>(repository: &'repo Repository, segment: &'_ Segment<'repo>,
                                           commit_id: Oid) {

    let iter = segment.iter(repository).unwrap();
    // here skip & find:
    let over = iter.skip_while(|x| x.as_ref().unwrap() != &commit_id );
    let mut peek = over.peekable();

    if peek.peek().is_none() {
        println!("Empty!");
        panic!("not found or last");
    } else {
        // todo: check the index
        // no unstaged changes?
        let to_apply = repository.find_commit(commit_id).unwrap();
        let new_oid = commit_cherry_picked(repository,
                                           &to_apply,
                                           &repository.head().unwrap().peel_to_commit().unwrap());
        // panic!("not supported currently");
        // commit
        // find where to resume

        // here we continue the whole sub-segment chain:
        debug!("now continue to pick the rest of the segment '{}'", segment.name());
        let commit = cherry_pick_commits(repository,
                                         peek.skip(1),
                                         repository.find_commit(new_oid).unwrap()).unwrap();

        // might need this if nothing to cherrypick anymore.
        segment.reset(repository, commit.id());

    }
}
// we cherry-pick on detached head. Unlike other tools.
//
fn rebase_segment_continue(repository: &Repository) -> RebaseResult {
    let path = marker_filename(repository);

    // todo: maybe this before calling this function?
    if ! fs::exists(&path).unwrap() {
        error!("marker file does not exist -- no segment is being rebased.");
        exit(1);
    }

    let content: String = fs::read_to_string(path).unwrap();
    let segment_name = content.trim();
    debug!("continue on {}", segment_name);

    if false {
        return rebase_continue_git1(repository, segment_name);
    } else {
        // native:
        // could be SKIP
        if let GitHierarchy::Segment(segment) = load(repository, &segment_name).unwrap() {

            if repository.state() == RepositoryState::CherryPick {
                // read the CHERRY_PICK_HEAD
                // todo: convert to step.step2...
                let commit_id  = Oid::from_str(
                    &fs::read_to_string(repository.commondir().join("CHERRY_PICK_HEAD")).unwrap().trim()).unwrap();

                debug!("should continue the cherry-pick {:?}", commit_id);

                continue_segment_cherry_pick(repository, &segment, commit_id);
            } else if repository.state() == RepositoryState::Clean {
                // the state
                segment.reset(repository,
                              repository.head().unwrap().peel_to_commit().unwrap().id());
            }

            let tmp_head: Branch<'_> = repository
                .find_branch(TEMP_HEAD_NAME, BranchType::Local)
                .unwrap();
            cleanup_segment_rebase(repository, &segment, tmp_head);
            return RebaseResult::Done;
        } else {
            panic!("segment not found");
        }
    }
}

// strange this mut. caller makes a copy?
fn drop_temporary_head(repository: &Repository, mut temp_head: Branch<'_>) {
    if true {
        temp_head.delete().expect("failed to delete a branch");
    } else {
        if !git_run(
            repository,
            &["branch", "-D", temp_head.name().unwrap().unwrap()],
        )
            .success()
        {
            panic!("branch -D failed");
        }
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
    return RebaseResult::Done;
}

fn force_head_to(repository: &Repository, name: &str, new_head: &Reference<'_>) {
    let oid = new_head.peel_to_commit().unwrap();
    // create it:
    debug!("relocating {:?} to {:?}", name, oid);
    repository.branch(name, &oid, true).unwrap();
    // git_run(repository, &["branch", "--force", segment.name(), new_head.name().unwrap()]);

    // checkout, since then I drop ...:
    let full_name = concatenate("refs/heads/", name);
    repository.set_head(&full_name).expect("failed to checkout");
    // git_run(repository, &["checkout", "--no-track", "-B", segment.name()]);
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

/// Compose commit message for the Sum/Merge of .... components given by the
/// first/others.
fn get_merge_commit_message<'a, 'b, 'c, Iter>(
    sum_name: &'b str,
    first: &'c str,
    others: Iter,
) -> String
where
    Iter: Iterator<Item = &'a str>,
{
    let mut message = format!("Sum: {sum_name}\n\n{}", first);

    const NAMES_PER_LINE: usize = 3;
    for (i, name) in others.enumerate() {
        message.push_str(" + ");
        message.push_str(name);

        if i % NAMES_PER_LINE == 0 {
            // exactly same as push_str()
            message += "\n"
        }
    }
    return message;
}

/// Given @sum, check if it's up-to-date.
///
/// If not: create a new git merge commit.
fn remerge_sum<'repo>(
    repository: &'repo Repository,
    sum: &Sum<'repo>,
    object_map: &HashMap<String, GitHierarchy<'repo>>, // this lifetime
) -> RebaseResult {
    let summands = sum.summands(repository);

/* assumption:
    sum has its summands   base/1 ... base/N
    these might resolve to References. -- how is that different from Branch?

    During the rebasing we change ... Branches (References), and update them in the `object_map'
    so we .... prefer to look up there.
*/

    // find the representation which we already have and keep updating.
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

    // convert to the nodes?
    debug!("The current parent commits are: {:?}", sum.parent_commits());
    for c in sum.parent_commits() {
        debug!("  {}", c);
    }

    let v = find_non_matching_elements(
        // iter2 - hash(iter1)
        graphed_summands.iter(), // these are &GitHierarchy

        sum.parent_commits().into_iter(), // Vec<Oid>
        // we get reference.
        // sum.reference.peel_to_commit().unwrap().parent_ids().into_iter(),
        |gh| {
            debug!("mapping {:?} to {:?}", gh.node_identity(),
                   gh.commit().id());
            gh.commit().id()
        }, // I get:  ^^^^^^^^^^^ expected `Oid`, found `Commit<'_>`
    );

    if v.is_empty() {
        debug!("sum is update: summands & parent commits align");
    } else {
        info!("so the sum is not up-to-date!");

        let first = graphed_summands.get(0).unwrap();

        let message = get_merge_commit_message(
            sum.name(),
            first.node_identity(), // : &GitHierarchy
            graphed_summands.iter()
                .skip(1).map(|x| x.node_identity()),
        );

        if graphed_summands.len() > 2 {
            let temp_head = checkout_new_head_at(repository, Some("temp-sum"), &first.commit())
                .unwrap();

            // use  git_run or?
            let mut cmdline = vec![
                "merge",
                "-m",
                &message, // why is this not automatic?
                "--rerere-autoupdate",
                "--strategy",
                "octopus",
                "--strategy",
                "recursive",
                "--strategy-option",
                "patience",
                "--strategy-option",
                "ignore-space-change",
            ];
            cmdline.extend(graphed_summands.iter().map(|s| s.node_identity()));

            git_run(repository, &cmdline);


            // "commit": move the SUM head with reflog message:
            force_head_to(
                repository,
                sum.name(),
                // have to sync
                repository
                    .find_branch("temp-sum", BranchType::Local)
                    .unwrap()
                    .get(),
            );
            // git_run("branch", "--force", sum.Name(), tempHead)
            debug!("delete: {:?}", temp_head.name());
            if !git_run(
                repository,
                &["branch", "-D", temp_head.name().unwrap().unwrap()],
            )
                .success()
            {
                panic!("branch -D failed");
            }

        } else {
            // libgit2

            assert!(checkout_new_head_at(repository, None, &first.commit()).is_none());

            // Options:
            let mut merge_opts = MergeOptions::new();
            merge_opts.fail_on_conflict(true)
                .standard_style(true)
                .ignore_whitespace(true)
                .patience(true)
                .minimal(true)
                ;

            let mut checkout_opts = CheckoutBuilder::new();
            checkout_opts.safe();


            // one more conversion:
            // we have Vec<GitHierarchy> ->  Vec<Commit> need..... Vec<AnnotatedCommit>
            //
            let annotated_commits : Vec<AnnotatedCommit<'_>> =
                graphed_summands.iter().skip(1).map(
                    |gh| {
                        let oid = gh.commit().id();
                        repository.find_annotated_commit(oid).unwrap()
                    }).collect();
            // the references vec:
            let annotated_commits_refs = annotated_commits.iter().collect::<Vec<_>>();

            debug!("Calling merge()");
            repository.merge(
                &annotated_commits_refs,
                Some(&mut merge_opts),
                Some(&mut checkout_opts)
            ).expect("Merge should succeed");

            // make if a function:
            // oid = save_index_with( message, signature);
            let mut index = repository.index().unwrap();
            if index.has_conflicts() {
                info!("SORRY conflicts detected");
                exit(1);
            }
            let id = index.write_tree().unwrap();
            let tree = repository.find_tree(id).unwrap();

            let sig = repository.signature().unwrap();


            // Create the commit:
            // another one: Reference -> Commit -> Oid >>> lookup >>> AnnotatedCommit->Oid
            let commits : Vec<Commit<'_>> = graphed_summands.iter()
                .map(|gh| gh.commit())
                .collect();
            // references
            let commits_refs = commits.iter().collect::<Vec<_>>();

            debug!("Calling merge()");

            let new_oid = repository.commit(
                Some("HEAD"),
                &sig, // author(),
                &sig, // committer(),
                &message,
                &tree,
                // this is however already stored in the directory:
                &commits_refs,
                // Error: "failed to create commit: current tip is not the first parent"
            ).unwrap();

            repository.cleanup_state().expect("cleaning up should succeed");

            // this both on the Repo/storer both here in our Data ?
            sum.reference.borrow_mut().set_target(new_oid, "re-merge")
                .expect("should update the symbolic reference -- sum");
        }
    }

    // do we have a hint -- another merge?
    // git merge

    return RebaseResult::Done;
}

/// Given full git-reference name /refs/remotes/xx/bb return xx and bb
fn extract_remote_name<'a>(name: &'a str) -> (&'a str, &'a str) {
    debug!("extract_remote_name: {:?}", name);
    // let norm = Reference::normalize_name(reference.name().unwrap(), ReferenceFormat::NORMAL).unwrap();

    let split_char = '/';

    let (prefix, rest) = name.split_once(split_char).unwrap();
    assert_eq!(prefix, "refs");
    let (prefix, rest) = rest.split_once(split_char).unwrap();
    assert_eq!(prefix, "remotes");

    let (remote, branch) = rest.split_once(split_char).unwrap();
    return (remote, branch);
}

fn fetch_upstream_of(repository: &Repository, reference: &Reference<'_>) -> Result<(), Error> {
    if reference.is_remote() {
        let (remote_name, branch) = extract_remote_name(reference.name().unwrap());
        let mut remote = repository.find_remote(remote_name).unwrap();
        debug!("fetching from remote {:?}: {:?}",
               remote.name().unwrap(),
               branch
        );

        // FetchOptions, message
        if remote.fetch(&[branch], None, Some("part of poset-rebasing")).is_err() {
            panic!("** Fetch failed");
        }
    } else if reference.is_branch() {
        let name = Reference::normalize_name(reference.name().unwrap(), ReferenceFormat::NORMAL).unwrap();

        // let b = Branch::wrap(*reference); // cannot move out of `*reference` which is behind a mutable reference
        info!("fetch local {name}");
        // why redo this? see above ^^
        let mut branch = repository
            .find_branch(extract_name(&name), BranchType::Local)
            .unwrap();

        let upstream = branch.upstream().unwrap();
        let upstream_name = upstream.name().unwrap().unwrap();

        // todo: check if still in sync, to not lose local changes.
        if git_same_ref(repository, reference, upstream.get()) {
            debug!("in sync, so let's fetch & update");
        } else {
            panic!("NOT in sync; should not update.");
            // or merge/rebase.
        }

        let (rem, br) = divide_str(upstream_name, '/');
        let mut remote = repository.find_remote(rem)?;

        info!("fetch {} {} ....", rem, br);
        if remote.fetch(&[br], None, None).is_ok() {
            let oid = branch
                .upstream()
                .unwrap()
                .get()
                .target()
                .expect("upstream disappeared");
            branch
                .get_mut()
                .set_target(oid, "fetch & fast-forward")
                .expect("fetch/sync failed");
        }
    }
    Ok(())
}

fn rebase_node<'repo>(
    repo: &'repo Repository,
    node: &GitHierarchy<'repo>,
    fetch: bool,
    object_map: &HashMap<String, GitHierarchy<'repo>>,
) {
    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(r) => {
            if fetch {
                fetch_upstream_of(repo, r).expect("fetch failed");
            }
        }
        GitHierarchy::Segment(segment) => {
            let my_span = span!(Level::INFO, "segment", name = segment.name());
            let _enter = my_span.enter();
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            remerge_sum(repo, sum, object_map);
        }
    }
}


// ancestor <---is parent-- ........ descendant
fn is_linear_ancestor<'repo>(repository: &'repo Repository, ancestor: Oid, descendant: Oid) -> bool
{
    if ancestor == descendant { return true;}

    let mut walk = repository.revwalk().unwrap();
    walk.push(descendant).expect("should set upper bound for Walk");
    // segment.reference.borrow().target().unwrap()
    walk.hide(ancestor).expect("should set the lower bound for Walk");
    walk.set_sorting(Sort::TOPOLOGICAL).expect("should set the topo ordering of the Walk");

    if walk.next().is_none() {
        return false;
    }

    for oid in walk {
        if repository.find_commit(oid.unwrap()).unwrap().parent_count() > 1 {
            panic!("a merge found");
        }
    }
    return true;
}


fn check_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>)
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

fn check_sum<'repo>(
    _repository: &'repo Repository,
    sum: &Sum<'repo>,
    _object_map: &HashMap<String, GitHierarchy<'repo>>, // this lifetime
) {
    // merge commit?
    let count = sum.reference.borrow().peel_to_commit().unwrap().parent_count();
    // terrible:
    // https://users.rust-lang.org/t/why-does-rust-use-the-same-symbol-for-bitwise-not-or-inverse-and-logical-negation/117337/2
    if !(count > 1) {
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


fn check_node<'repo>(
    repo: &'repo Repository,
    node: &GitHierarchy<'repo>,
    object_map: &HashMap<String, GitHierarchy<'repo>>,
) {
    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(_r) => {
            // no
        }
        GitHierarchy::Segment(segment) => {
            check_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            check_sum(repo, sum, object_map);
        }
    }
}


fn start_rebase(repository: &Repository, root: String, fetch: bool, ignore: &Vec<String> ) {
    // summand -> object_map ->
    let (
        object_map,    // String -> GitHierarchy
        hash_to_graph, // stable graph:  String -> index ?
        graph,         // index -> String?
        discovery_order,
    ) = find_hierarchy(repository, root);

    for v in &discovery_order {
        let name = object_map.get(v).unwrap().node_identity();
        println!(
            "{:?} {:?} {:?}",
            v,
            name,
            graph
                .node_weight(hash_to_graph.get(v).unwrap().clone())
                .unwrap()
        );
        if ignore.iter().find(|x| x == &name).is_some() {
            eprintln!("found to be ignored {name}");
            continue;
        }
        let vertex = object_map.get(v).unwrap();
        check_node(repository, vertex, &object_map);
    }

    for v in discovery_order {
        println!(
            "{:?} {:?} {:?}",
            v,
            object_map.get(&v).unwrap().node_identity(),
            graph
                .node_weight(hash_to_graph.get(&v).unwrap().clone())
                .unwrap()
        );
        let vertex = object_map.get(&v).unwrap();
        rebase_node(repository, vertex, fetch, &object_map);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short = 'g')]
    directory: Option<PathBuf>,
    #[arg(short, long)]
    fetch: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short, long = "continue")]
    cont: bool,
    root_reference: Option<String>,

    #[arg(short, long = "ignore")]
    ignore: Vec<String>
}

///
fn main() {
    let cli = Cli::parse();
    // cli can override the Env variable.
    init_tracing(cli.verbose);

    let repo = match Repository::open(cli.directory.unwrap_or(std::env::current_dir().unwrap())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    // todo:
    // normalize_name(refname: &str, flags: ReferenceFormat) -> Result<String, Error> {

    let root = cli.root_reference
        // if in detached HEAD -- will panic.
        .unwrap_or_else(|| repo.head().unwrap().name().unwrap().to_owned());

    let root = GitHierarchy::Name(root); // not load?
    println!("root is {}", root.node_identity());

    if cli.cont {
        rebase_segment_continue(&repo);
    }

    start_rebase(&repo, root.node_identity().to_owned(), cli.fetch,
                 &cli.ignore);
}
