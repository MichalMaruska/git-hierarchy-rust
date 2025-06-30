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

use ::git_hierarchy::base::{get_repository,set_repository,unset_repository};
use ::git_hierarchy::permutation::reorder_by_permutation;

// I need both:
use ::git_hierarchy::git_hierarchy::{GitHierarchy,Segment,Sum};

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/


use ::git_hierarchy::graph;
use graph::discover::NodeExpander;
use graph::topology_sort::topological_sort;


enum RebaseResult {
    Nothing,
    Done,
    Failed,
}


// todo: method
//
fn rebase_segment(repo: &Repository, segment: &Segment) -> RebaseResult {
    warn!("should rebase");

        println!("{}", vertex.node_identity());
    return RebaseResult::Failed;
}

fn fetch_upstream_of(repository: &Repository, reference: &Reference) {
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
        let branch = repository.find_branch(&name, BranchType::Local).unwrap();

        // let b = Branch::wrap(*reference); // cannot move out of `*reference` which is behind a mutable reference
        branch.upstream();
        // is is a
    // branch -> find remote
    // load config, see
    // double check if still in sync, then
    }
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
