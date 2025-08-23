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

use ::discover_graph::GraphDiscoverer;
use ::git_hierarchy::graph::discover::NodeExpander;
use ::git_hierarchy::graph::discover_pet::GitHierarchyProvider;

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

    // 1. Perform discovery
    let provider = GitHierarchyProvider::new(get_repository());
    let mut discoverer = GraphDiscoverer::new(provider);
    let discovery_order = discoverer.dfs_discover(root);

    let graph = discoverer.get_graph();
    let (provider, hash_to_graph) = discoverer.get_provider();

    for v in discovery_order {
        println!("{:?} {:?} {:?}", v,
                 provider.object_map.get(&v).unwrap().node_identity(),
                 graph.node_weight(
                     hash_to_graph.get(&v).unwrap().clone()).unwrap()
        );
    }

    unset_repository();
}
