#![deny(elided_lifetimes_in_paths)]
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

use clap::Parser;
use git2::{Repository,Reference,Error,Branch,BranchType,ReferenceFormat};

use tracing::{warn,info,debug};

use ::git_hierarchy::base::{get_repository,set_repository,unset_repository,git_same_ref,checkout_new_head_at};
use ::git_hierarchy::utils::{extract_name,divide_str,concatenate};
use ::git_hierarchy::execute::git_run;

use crate::graph::discover_pet::find_hierarchy;

// I need both:
#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy,Segment,Sum};

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph;
use graph::discover::NodeExpander;


fn rebase_node<'repo>(repo: &Repository, node: &GitHierarchy<'_>, fetch: bool) {
    match node {
        GitHierarchy::Name(_n) => {panic!();}
        GitHierarchy::Reference(r) => {
            if fetch {
                fetch_upstream_of(repo, r);
            }}
        GitHierarchy::Segment(segment)=> {
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(_sum) => {
            warn!("should re-merge");
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
        rebase_node(repository, vertex, fetch);
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
fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt::init();

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
    set_repository(repo);

    let repo = get_repository();

    // todo:
    // normalize_name(refname: &str, flags: ReferenceFormat) -> Result<String, Error> {
    // load one Segment:

    let root = cli.root_reference.unwrap_or_else(
        || repo.head().unwrap().name().unwrap().to_owned());
    let root = GitHierarchy::Name(root);

    println!("root is {}", root.node_identity());

    start_rebase(repo, root.node_identity().to_owned(),
                 cli.fetch);

    unset_repository();
}
