// walk the hierarchy
// - assemble list of segments/sums.
// todo:
// - clone
// - replaceInHierarchy ...the base from->to, mapping

//
// get list of segments
use git2::{Repository,Reference,Error};
use clap::Parser;
// use std::error::Error;
use log::{self,info,error};
use stderrlog::LogLevelNum;
// use tracing::{Level, event, instrument};

use ::git_hierarchy::base::{get_repository,set_repository,unset_repository};

use ::git_hierarchy::*;
/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use crate::git_hierarchy::*;

use ::git_hierarchy::graph;
use graph::discover::NodeExpander;
use graph::topology_sort::topological_sort;

// use std::path::PathBuf;
use tracing::debug;
use tracing_subscriber;

// error: cannot find derive macro `Parser` in this scope
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
    let mut root = GitHierarchy::Name(cli.root_reference.unwrap_or("mmc".to_string()));
    println!("root is {}", root.node_identity());
    let (graph, vertices ) =
        graph::discover::discover_graph(vec!(Box::new(root)));

    let order = graph.toposort();
    for i in &order {
        println!("{i} {}", vertices[*i].node_identity());
    }

    // let msg = repo.message();
    // println!("{:?}", &head);

    unset_repository();
}
