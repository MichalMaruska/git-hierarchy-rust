// walk the hierarchy
// - assemble list of segments/sums.
// todo:
// - clone
// - replaceInHierarchy ...the base from->to, mapping

use clap::Parser;
use git2::Repository;

use std::collections::HashMap;
use std::path::PathBuf;

use ::git_hierarchy::utils::init_tracing;
/*
 note: ambiguous because of a conflict between a name from a glob
       import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph::discover::NodeExpander;
use ::git_hierarchy::graph::discover_pet::{find_hierarchy};

#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy, Segment, Sum, load};

#[allow(unused)]
use tracing::{debug, info};

fn process_node<'repo>(
    repository: &'repo Repository,
    node: &GitHierarchy<'repo>,
    _object_map: &HashMap<String, GitHierarchy<'repo>>, // _remapped : HashMap<String, String>,
) {
    println!(
        "{:?}",
        // object_map.get(&v).unwrap()
        node.node_identity(),
        // object_map
        // graph.node_weight(hash_to_graph.get(node).unwrap().clone()).unwrap()
    );

    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(r) => {
            println!("a ref");
        }
        GitHierarchy::Segment(segment) => {
            let base = segment.base(repository);
            let start = &segment._start;
            // start == base.peel_to_commit().unwrap())

            // target
            let state;
            if base.peel_to_commit().unwrap().id() == start.target().unwrap() {
                state = "up-to-date";
            } else {
                state = "need-rebase";
            }
            println!(
                "segment {:?} on , {:?}",
                base.name().unwrap(),
                state // base.peel_to_commit().unwrap().id(),
            );
        }
        GitHierarchy::Sum(sum) => {
            let summands = sum.summands(repository);

            println!("a sum of: ");
            for s in &summands {
                println!("{}", s.node_identity());
            }
        }
    }
}

fn walk_down<F>(repository: &Repository, root: &str, mut process: F)
where
    F: for<'repo, 'a> FnMut(
        &'repo git2::Repository,
        &GitHierarchy<'repo>,
        &'a HashMap<String, GitHierarchy<'repo>>,
    ) -> (), // F : FnMut(&Repository, GitHierarchy,
             //          &HashMap<String, GitHierarchy>) -> ()
{
    let (
        object_map,     // String -> GitHierarchy
        _hash_to_graph, // stable graph:  String -> index ?
        _graph,         // index -> String?
        discovery_order,
    ) = find_hierarchy(repository, root.to_owned());

    // convert the gh objects?
    for v in discovery_order {
        let vertex = object_map.get(&v).unwrap();
        process(repository, vertex, &object_map);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short='g')]
    directory: Option<PathBuf>,

    root_reference: Option<String>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() {
    let cli = Cli::parse();

    init_tracing(cli.verbose);

    let repo = match Repository::open(cli.directory.unwrap_or(std::env::current_dir().unwrap())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    let root = cli
        .root_reference
        .unwrap_or_else(|| repo.head().unwrap().name().unwrap().to_owned());

    walk_down(&repo, &root, process_node);
}
