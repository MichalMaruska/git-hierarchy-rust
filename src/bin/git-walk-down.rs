// walk the hierarchy
// - assemble list of segments/sums.
// todo:
// - clone
// - replaceInHierarchy ...the base from->to, mapping

use clap::Parser;
use git2::{Repository,};
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
use tracing::{debug, info};

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

    // load one Segment:
    let root = cli.root_reference.unwrap();

    let (object_map, // String -> GitHierarchy
         hash_to_graph,  // stable graph:  String -> index ?
         graph,          // index -> String?
         discovery_order) = find_hierarchy(&repo, root);


    // convert the gh objects?
    for v in discovery_order {
        println!("{:?} {:?} {:?}", v,
                 object_map.get(&v).unwrap().node_identity(),
                 graph.node_weight(
                     hash_to_graph.get(&v).unwrap().clone()).unwrap()
        );
    }
}
