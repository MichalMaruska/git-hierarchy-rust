// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

//
// get list of segments
use git2::{Repository,Reference,Error,Branch,BranchType,ReferenceFormat};
use clap::Parser;
// use std::error::Error;

#[allow(unused)]
use log::{self,info,warn,error,debug};
use stderrlog::LogLevelNum;
// use tracing::{Level, event, instrument};

// This declaration will look for a file named `graph'.rs and will
// insert its contents inside a module named `my` under this scope

use ::git_hierarchy::base::{get_repository,set_repository,unset_repository,git_same_ref};
use ::git_hierarchy::permutation::reorder_by_permutation;
use ::git_hierarchy::utils::{extract_name,divide_str};
use ::git_hierarchy::execute::git_run;

// I need both:
use ::git_hierarchy::git_hierarchy::{GitHierarchy,Segment,Sum};

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/


use ::git_hierarchy::graph;
use ::git_hierarchy::utils::concatenate;
use graph::discover::NodeExpander;
use graph::topology_sort::topological_sort;


enum RebaseResult {
    Nothing,
    Done,
    Failed,
}


fn rebase_segment(repository: &Repository, segment: &Segment) -> RebaseResult {
    warn!("should rebase {}", segment.name());

    // persistent mark, if we fail, and during the session.
    /*
    mark := plumbing.NewSymbolicReference(".segment-cherry-pick", segment.Ref.Name());
    err := repository.Storer.SetReference(mark)
    */

    let new_start = segment.base(repository);
    if git_same_ref(repository, &new_start, &segment._start ) {
        info!("nothing to do");
        return RebaseResult::Nothing;
    }

    // todo: segment_empty()
    if git_same_ref(repository, &segment.reference, &segment._start) {
        rebase_empty_segment(segment);
        return RebaseResult::Done;
    }

    // const
    let temp_head = "temp-segment";
    Branch::name_is_valid(&temp_head).unwrap();
    debug!("rebasing by Cherry-picking {}!", segment.name());

    // checkout to that ref
    // todo: git stash
    // must change to the directory!
    git_run(repository, &["checkout", "--no-track", "-B", temp_head, new_start.name().unwrap()]);

    // segment git-range
    let mut what_to_cherrypick = concatenate(segment._start.name().unwrap(), "..");
    what_to_cherrypick.push_str(segment.reference.name().unwrap());

    if !git_run(repository, &["cherry-pick", &what_to_cherrypick] ).success() {
        panic!("cherry-pick failed");
    }

    let status = rebase_segment_finish(repository, segment,
                                       repository.find_branch(&temp_head, BranchType::Local).unwrap().get()
    );

    git_run(repository, &["branch", "--delete", temp_head]);
    return status;
}

fn rebase_empty_segment(segment: &Segment) {
    debug!("rebase empty segment: {}", segment.name());
    // fixme:  move Start to Base!

    //segment.ResetStart()
    // setReferenceTo(TheRepository, segment.Ref, segment.Base)
}

fn rebase_segment_finish(repository: &Repository, segment: &Segment, new_head: &Reference) -> RebaseResult {
    // unimplemented!("Remote");
    segment.reset(); // fixme!
    // reflog etc.
    git_run(repository, &["branch", "--force", segment.name(), new_head.name().unwrap()]);
    git_run(repository, &["checkout", "--no-track", "-B", segment.name()]);

    return RebaseResult::Done;
}


fn fetch_upstream_of(repository: &Repository, reference: &Reference) -> Result<(), Error>{
    warn!("should fetch");
    // remote ->
    if reference.is_remote() {
        // mmc: I think it's dangerous ... better avoid using this.
        // let remote: RemoteHead;
        // just fetch
        // Remote.fetch()
        unimplemented!("Remote");
    } else if reference.is_branch() {
        let name = Reference::normalize_name(reference.name().unwrap(), ReferenceFormat::NORMAL).unwrap();
        warn!("fetch local {name}");
        let mut branch = repository.find_branch(extract_name(&name), BranchType::Local).unwrap();
        // let b = Branch::wrap(*reference); // cannot move out of `*reference` which is behind a mutable reference
        let upstream = branch.upstream().unwrap();
        // todo: double check if still in sync, then
        let upstream_name = upstream.name().unwrap().unwrap();

        // and this is host/branch
        // fixme:
        if git_same_ref(repository, reference, upstream.get()) {
            info!("in sync");
        } else {
            warn!("NOT in sync");
        }
        //
        let (rem, br) = divide_str(upstream_name, '/');
        let mut remote = repository.find_remote(rem)?;
        // repo.find_remote("origin")?.fetch(&["main"], None, None)
        if true {
            warn!("fetch {} {} ....", rem, br);
            if remote.fetch(&[br], None, None).is_ok() {
                let oid = branch.upstream().unwrap().get().target().expect("upstream disappeared");
                branch.get_mut().set_target(oid, "fetch"); // & fast-forward ?
            }
        }
        // sync the local
    }
    Ok(())
}


fn rebase_node(repo: &Repository, node: &GitHierarchy, fetch: bool) {
    match node {
        GitHierarchy::Name(_n) => {panic!();}
        GitHierarchy::Reference(r) => {
            if fetch {
                fetch_upstream_of(repo, r);
            }}
        GitHierarchy::Segment(segment)=> {
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            warn!("should re-merge");
        }
    }
}

fn start_rebase(repo: &Repository, vec: Vec<Box<dyn NodeExpander>>, fetch: bool) {
    let (graph, mut vertices) =
       graph::discover::discover_graph(vec);

    let order = graph.toposort();
    reorder_by_permutation(&mut vertices, &order);

    while !vertices.is_empty() {
        let boxed = vertices.pop().unwrap();
        let vertex = boxed.as_any().downcast_ref::<GitHierarchy>().unwrap();

        println!("{}", vertex.node_identity());

        rebase_node(repo, vertex, fetch);
    }
}

// error: cannot find derive macro `Parser` in this scope
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    directory: Option<String>,
    root_reference: Option<String>,
    #[arg(short, long)]
    fetch: bool,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

///
// fn refsWithPrefixIter(iterator storer.ReferenceIter, prefix string) storer.ReferenceIter {

fn main() {
    let cli = Cli::parse();

    stderrlog::new().module(module_path!())
        .module("git_hierarchy")
        .verbosity(LogLevelNum::from(cli.verbose as usize)) // Cli.verbose Warn Info LogLevelNum::Info
        .init()
        .unwrap();

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
    set_repository(repo);

    let repo = get_repository();
    let head = repo.head();

    // todo:
    // normalize_name(refname: &str, flags: ReferenceFormat) -> Result<String, Error> {

    // load one Segment:
    let mut root = GitHierarchy::Name(cli.root_reference.unwrap_or("mmc".to_string()));
    println!("root is {}", root.node_identity());

    start_rebase(repo, vec!(Box::new(root)), cli.fetch);

    unset_repository();
}
