// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

//
// get list of segments
use git2::{Repository,Reference,Error};
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
use ::git_hierarchy::git_hierarchy::GitHierarchy;

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/


use ::git_hierarchy::graph;
use graph::discover::NodeExpander;
use graph::topology_sort::topological_sort;


fn start_rebase(vec: Vec<Box<dyn NodeExpander>>, fetch: bool) {
    let (graph, vertices ) =
        graph::discover::discover_graph(vec);

    let order = graph.toposort();
    reorder_by_permutation(&mut vertices, &order);

    while !vertices.is_empty() {
        let boxed = vertices.pop().unwrap();
        let vertex = boxed.as_any().downcast_ref::<GitHierarchy>().unwrap();

        println!("{}", vertex.node_identity());

        match vertex {
            GitHierarchy::Name(_n) => {panic!();}
            GitHierarchy::Reference(r) => {
                if !fetch {}
            }
            GitHierarchy::Segment(segment)=> {}
            GitHierarchy::Sum(sum) => {}
        }
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
