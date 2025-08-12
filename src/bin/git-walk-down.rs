// walk the hierarchy
// - assemble list of segments/sums.
// todo:
// - clone
// - replaceInHierarchy ...the base from->to, mapping

use clap::Parser;

use tracing_subscriber;

use git2::{Repository,};
use ::git_hierarchy::base::{set_repository,get_repository,unset_repository};

/*
 note: ambiguous because of a conflict between a name from a glob
       import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph::discover::NodeExpander;
use ::git_hierarchy::graph::discover_pet::{find_hierarchy};

#[allow(unused)]
use tracing::debug;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    directory: Option<String>,
    root_reference: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    set_repository(repo);

    // load one Segment:
    let root = cli.root_reference.unwrap();

    let (object_map, // String -> GitHierarchy
         hash_to_graph,  // stable graph:  String -> index ?
         graph,          // index -> String?
         discovery_order) = find_hierarchy(get_repository(), root);

    // convert the gh objects?
    for v in discovery_order {
        println!("{:?} {:?} {:?}", v,
                 object_map.get(&v).unwrap().node_identity(),
                 graph.node_weight(
                     hash_to_graph.get(&v).unwrap().clone()).unwrap()
        );
    }

    unset_repository();
}
