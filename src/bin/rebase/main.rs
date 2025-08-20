#![deny(elided_lifetimes_in_paths)]
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

use clap::Parser;
use git2::{Repository,Reference,Error,Branch,BranchType,ReferenceFormat,Remote};

use tracing::{warn,info,debug};

use std::collections::{HashMap};
use std::iter::Iterator;
use ::git_hierarchy::base::{get_repository,set_repository,unset_repository,git_same_ref,checkout_new_head_at};
use ::git_hierarchy::utils::{extract_name,divide_str,concatenate,find_non_matching_elements};
use ::git_hierarchy::execute::git_run;

use crate::graph::discover_pet::find_hierarchy;

// I need both:
#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy,Segment,Sum,load};

use std::fs;
use std::io;
use std::path::PathBuf;

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

const TEMP_HEAD_NAME : &str = "tempSegment";
const MARKER_FILENAME : &str = ".segment-cherry-pick";

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

// either exit or rewrite the segment ....its reference should update oid.
fn rebase_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>) -> RebaseResult {
    if segment.uptodate(repository) {
        info!("nothing to do -- base and start equal");
        return RebaseResult::Nothing;
    }

    let new_start = segment.base(repository);

    // todo: segment_empty()
    if segment.empty(repository)  {
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
    let temp_head = checkout_new_head_at(repository, temp_head,
                            &new_start.peel_to_commit().unwrap());

    if !git_run(repository, &["cherry-pick", segment.git_revisions().as_str() ]).success() {
        // return RebaseResult::Failed;
        panic!("cherry-pick failed");
    }

    // I have to re-find it?
    rebase_segment_finish(repository, segment,
                                       // temp_head.get()
                                       repository.find_branch(&TEMP_HEAD_NAME, BranchType::Local).unwrap().get());
    cleanup_segment_rebase(repository, segment, temp_head);
    return RebaseResult::Done;
}

fn rebase_segment_continue(repository: &Repository) -> RebaseResult {
    let path = marker_filename(repository);

    if fs::exists(&path).unwrap() {
        let name :String = fs::read_to_string(path).unwrap();
        debug!("continue on {}", name);
        if !git_run(repository, &["cherry-pick", "--continue"]).success() {
            info!("Good?")
            // panic!("cherry-pick failed");
        }
        if let GitHierarchy::Segment(segment) = load(repository, &name).unwrap() {

            let tmp_head : Branch<'_> = repository.find_branch(TEMP_HEAD_NAME, BranchType::Local).unwrap();
            if tmp_head.is_head() {
                //name: &str, branch_type: BranchType) -> Result<Branch<'_>, Error> {head();
                rebase_segment_finish(repository, &segment,
                                                   repository.find_branch(&TEMP_HEAD_NAME, BranchType::Local).unwrap().get());
                cleanup_segment_rebase(repository, &segment, tmp_head);
                return RebaseResult::Done;
            } else {
                // mismatch
                panic!();
            }
        } else {
            panic!();
        }
    } else {
     RebaseResult::Nothing
    }
}

// bad name:
fn cleanup_segment_rebase(repository: &Repository, _segment: &Segment<'_>, temp_head: Branch<'_> ) {
    debug!("delete: {:?}", temp_head.name());
    if !git_run(repository, &["branch", "-D", temp_head.name().unwrap().unwrap() ]).success() {
        panic!("branch -D failed");
    }
    // temp_head.delete().expect("failed to delete a branch");

    let path = marker_filename(repository);
    debug!("delete: {:?}", path);
    fs::remove_file(path).unwrap();
}

fn rebase_empty_segment<'repo>(segment: &Segment<'repo>, repository: &'repo Repository) -> RebaseResult {
    debug!("rebase empty segment: {}", segment.name());
    // fixme:  move Start to Base!
    segment.reset(repository);
    return RebaseResult::Done;
}

fn force_head_to(repository: &Repository, name: &str, new_head: &Reference<'_>) {
    debug!("relocating {:?} to {:?}", name, new_head.name().unwrap());
    let oid = new_head.peel_to_commit().unwrap();
    // create it:
    repository.branch(name, &oid, true).unwrap();
    // git_run(repository, &["branch", "--force", segment.name(), new_head.name().unwrap()]);

    // checkout, since then I drop ...:
    let full_name = concatenate("refs/heads/",  name);
    repository.set_head(&full_name).expect("failed to checkout");
    // git_run(repository, &["checkout", "--no-track", "-B", segment.name()]);
}

fn rebase_segment_finish<'repo>(repository: &'repo Repository, segment: &Segment<'repo>, new_head: &Reference<'_>) {
    segment.reset(repository);

    // reflog etc.
    force_head_to(repository, segment.name(), new_head);
}


// lifetime irrelevant?
fn get_merge_commit_message<'a,'b,'c,Iter>(sum_name: &'b str, first: &'c str, others: Iter) -> String
    where
    Iter : Iterator<Item = &'a str>
{
    let mut message = format!("Sum: {sum_name}\n\n{}", first);

    const NAMES_PER_LINE : usize = 3;
    for (i, name) in others.enumerate() {
        // resolve them! maybe sum.Summands should be a map N -> ref
        // pointerRef, _ := TheRepository.Reference(ref.Target(), false)
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
fn remerge_sum(repository: &Repository, sum: &Sum<'_>, object_map: &HashMap<String, GitHierarchy<'_>>) -> RebaseResult {

    let summands = sum.summands(repository);

    let mut graphed_summands : Vec<&GitHierarchy<'_>> = summands.iter().map(
        |s| {
            let gh = object_map.get(s.node_identity()).unwrap();
            debug!("convert {:?} to {:?}", s.node_identity(), gh.node_identity());
            gh
        }
        // here we might use object_map
        // vec<GitHierarchy> not Name but real.
    ).collect();

    // Vec<GitHierarchy<'repo>> is useless.
    // convert to the nodes?

    let v = find_non_matching_elements(
        graphed_summands.iter(),   // these are <&GitHierarchy>

        // we get reference.
        // sum.reference.peel_to_commit().unwrap().parent_ids().into_iter(),
        sum.parent_commits().into_iter(),
        //
        |gh|{
            debug!("mapping {:?}", gh.node_identity());
            gh.commit().id() }
        // I get:  ^^^^^^^^^^^ expected `Oid`, found `Commit<'_>`
    );

    if ! v.is_empty() {
        info!("so the sum is not up-to-date!");

        let first = graphed_summands.remove(0);
        // &graphed_summands[0];
        let others = graphed_summands.iter(); // .skip(1)
        // let v = vec!();

        #[allow(unused)]
        let message = get_merge_commit_message(sum.name(),
                                               first.node_identity(),
                                               // : &GitHierarchy
                                               others.map(|x | x.node_identity()));
        // proceed:
        #[allow(unused)]
        let temp_head = checkout_new_head_at(repository,"temp-sum", &first.commit());

        // use  git_run or?
        let mut cmdline = vec!["merge",
                               "-m", &message, // why is this not automatic?
                               "--rerere-autoupdate",
                               "--strategy", "octopus",
                               "--strategy", "recursive",
                               "--strategy-option", "patience",
                               "--strategy-option", "ignore-space-change",
        ];
        for s in graphed_summands {
            cmdline.push(s.node_identity());
        }

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
        force_head_to(repository, sum.name(),
                      // have to sync
                      repository.find_branch("temp-sum", BranchType::Local).unwrap().get());
        // git_run("branch", "--force", sum.Name(), tempHead)

        debug!("delete: {:?}", temp_head.name());
        if !git_run(repository, &["branch", "-D", temp_head.name().unwrap().unwrap() ]).success() {
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


fn rebase_node<'repo>(repo: &'repo Repository,
                      node: &GitHierarchy<'repo>,
                      fetch: bool,
                      object_map: &HashMap<String, GitHierarchy<'repo>>) {
    match node {
        GitHierarchy::Name(_n) => {panic!();}
        GitHierarchy::Reference(r) => {
            if fetch {
                fetch_upstream_of(repo, r).expect("fetch failed");
            }}
        GitHierarchy::Segment(segment)=> {
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            remerge_sum(repo, sum, object_map);
        }
    }
}


fn start_rebase(repository: &Repository,
                root: String,
                fetch: bool) {

    let (object_map, // String -> GitHierarchy
         hash_to_graph,  // stable graph:  String -> index ?
         graph,          // index -> String?
         discovery_order) = find_hierarchy(repository, root);

    for v in discovery_order {
        println!("{:?} {:?} {:?}", v,
                 object_map.get(&v).unwrap().node_identity(),
                 graph.node_weight(
                     hash_to_graph.get(&v).unwrap().clone()).unwrap()
        );
        let vertex = object_map.get(&v).unwrap();
        rebase_node(repository, vertex, fetch, &object_map);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short='g')]
    directory: Option<String>,
    #[arg(short, long)]
    fetch: bool,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short, long="continue")]
    cont: bool,
    root_reference: Option<String>,
}

///
fn main() {

    let cli = Cli::parse();
    // cli can override the Env variable.
    if cli.verbose > 0 {
        tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();
    } else {
        tracing_subscriber::fmt::init();
    }

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
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
