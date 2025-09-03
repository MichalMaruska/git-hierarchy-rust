#![deny(elided_lifetimes_in_paths)]
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

use clap::Parser;
use git2::{Branch, BranchType, Error, Commit, Reference, ReferenceFormat, Repository,
           MergeOptions, CherrypickOptions,
           build::CheckoutBuilder,
           Oid,
           RepositoryState,
};

#[allow(unused)]
use tracing::{debug, info, warn};

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

use std::fs;
use std::io;
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

                      let tree;

                      // use `cherrypick'

                      info!("cherry-pick commit: {:?}", to_apply);

                      let mut checkout_opts = CheckoutBuilder::new();
                      checkout_opts.safe();

                      let mut cherrypick_opts = CherrypickOptions::new();
                      cherrypick_opts.checkout_builder(checkout_opts);

                      let result = repository.cherrypick(&to_apply, Some(&mut cherrypick_opts));

                      if result.is_ok() {
                          // todo: see if conflicts ....
                          let mut index = repository.index().unwrap();
                          if index.has_conflicts() {
                              eprintln!("SORRY conflicts detected");
                              // so we have .git/CHERRY_PICK_HEAD ?
                              exit(1);
                          }

                          if index.is_empty() {
                              eprintln!("SORRY nothing staged, empty -- skip?");
                              // so we have .git/CHERRY_PICK_HEAD ?
                              exit(1);
                          }

                          let id = index.write_tree().unwrap();
                          //  "cannot create a tree from a not fully merged index."
                          tree = repository.find_tree(id).unwrap();
                      } else {
                          eprintln!("cherrypick failed {:?}", result.err());
                          let index = repository.index().unwrap();
                          if index.has_conflicts() {
                              eprintln!("SORRY conflicts detected");
                          }
                          panic!();
                      }

                      let new_oid = repository.commit(
                          Some("HEAD"),
                          // copy over:
                          &to_apply.author(),
                          &to_apply.committer(),
                          &to_apply.message().unwrap(),
                          // and timestamps? part of those ^^ !
                          &tree,
                          &[&base_commit],
                      ).unwrap();

                      repository.cleanup_state().unwrap();
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
    create_marker_file(repository, segment.name()).unwrap();

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

fn rebase_segment_continue(repository: &Repository) -> RebaseResult {
    let path = marker_filename(repository);

    if ! fs::exists(&path).unwrap() {
        panic!("not segment is being rebased.");
    }

    let segment_name: String = fs::read_to_string(path).unwrap();
    debug!("continue on {}", segment_name);

    if false {
        if !git_run(repository, &["cherry-pick", "--continue"]).success() {
            info!("Good?")
            // panic!("cherry-pick failed");
        }

        if let GitHierarchy::Segment(segment) = load(repository, &segment_name).unwrap() {
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

    } else {
        // native:
        // could be SKIP
        if repository.state() != RepositoryState::CherryPick {
            panic!("unexpected repository state");
        }
        // read the CHERRY_PICK_HEAD
        let commit_id  = Oid::from_str(
            &fs::read_to_string(repository.commondir().join("CHERRY_PICK_HEAD")).unwrap().trim()).unwrap();

        debug!("should resume rebasing segment {} from {:?}", segment_name, commit_id);

        if let GitHierarchy::Segment(segment) = load(repository, &segment_name).unwrap() {
            let iter = segment.iter(repository).unwrap();

            let over = iter.skip_while(|x| x.as_ref().unwrap() != &commit_id );

            let mut peek = over.peekable();
            // let found = iter.find

            if peek.peek().is_none() {
                println!("Empty!");
                panic!("not found or last");
            } else {
                // todo: check the index
                // no unstaged changes?
                let to_apply = repository.find_commit(commit_id).unwrap();


                let mut index = repository.index().unwrap();
                if index.has_conflicts() {
                    eprintln!("SORRY conflicts detected");
                    // so we have .git/CHERRY_PICK_HEAD ?
                    exit(1);
                }

                let id = index.write_tree().unwrap();
                //  "cannot create a tree from a not fully merged index."
                let tree = repository.find_tree(id).unwrap();
                let new_oid = repository.commit(
                    Some("HEAD"),
                    // copy over:
                    &to_apply.author(),
                    &to_apply.committer(),
                    &to_apply.message().unwrap(),
                    // and timestamps? part of those ^^ !
                    &tree,
                    &[&repository.head().unwrap().peel_to_commit().unwrap()],
                ).unwrap();

                repository.cleanup_state().unwrap();
                // panic!("not supported currently");
                // commit
                // find where to resume

                let commit = cherry_pick_commits(repository,
                                                 peek,
                                                 repository.find_commit(new_oid).unwrap()).unwrap();
                segment.reset(repository, commit.id());
            }

            //
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

// bad name:
fn cleanup_segment_rebase(repository: &Repository, _segment: &Segment<'_>, mut temp_head: Branch<'_>) {

    debug!("delete: {:?} {}", temp_head.name().unwrap().unwrap(),
           temp_head.get().target().unwrap());

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
                let gh = object_map.get(s.node_identity()).unwrap();
                debug!(
                    "convert {:?} to {:?}",
                    s.node_identity(),
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

    if !v.is_empty() {
        info!("so the sum is not up-to-date!");

        let first = graphed_summands.get(0).unwrap();

        let message = get_merge_commit_message(
            sum.name(),
            first.node_identity(), // : &GitHierarchy
            graphed_summands.iter()
                .skip(1).map(|x| x.node_identity()),
        );

        // proceed:
        let temp_head = checkout_new_head_at(repository,"temp-sum", &first.commit());

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
        cmdline.extend(graphed_summands.iter().skip(1).map(|s| s.node_identity()));

        git_run(repository, &cmdline);

        /*
            piecewise

            otherNames := lo.Map(others,
            func (ref *plumbing.Reference, _ int) string {
            return ref.Name().String()})

            // otherNames...  cannot use otherNames (variable of type []string) as []any value in argument to
            fmt.Println("summands are:", first, otherNames)

            if piecewise {
            // reset & retry
            // piecewise:
            for _, next := range others {
            gitRun("merge", "-m",
            "Sum: " + next.Name().String() + " into " + sum.Name(),
            "--rerere-autoupdate", next.Name().String())
        */

        // finish
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
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            remerge_sum(repo, sum, object_map);
        }
    }
}

fn start_rebase(repository: &Repository, root: String, fetch: bool) {
    // summand -> object_map ->
    let (
        object_map,    // String -> GitHierarchy
        hash_to_graph, // stable graph:  String -> index ?
        graph,         // index -> String?
        discovery_order,
    ) = find_hierarchy(repository, root);

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

    start_rebase(&repo, root.node_identity().to_owned(), cli.fetch);
}
