#![allow(unused)]

// deny, warn, allow...

// #![warn(unused_imports)]
//# allow

// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

//
// get list of segments
use git2::{Repository,Reference,Error};
use clap::Parser;
// use std::error::Error;
use log::{self,info,error};
use stderrlog::LogLevelNum;
// use tracing::{Level, event, instrument};

// This declaration will look for a file named `graph'.rs and will
// insert its contents inside a module named `my` under this scope

mod base;
use base::{get_repository,set_repository,unset_repository};

mod utils;

mod git_hierarchy;
use git_hierarchy::*;

mod graph;
use graph::discover::NodeExpander;


// use std::path::PathBuf;

// error: cannot find derive macro `Parser` in this scope
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    directory: Option<String>
}


const GLOB_REFS_BASES: &str = "refs/base/*";

///
// fn refsWithPrefixIter(iterator storer.ReferenceIter, prefix string) storer.ReferenceIter {

fn main() {
    let cli = Cli::parse();

    stderrlog::new().module(module_path!())
        .verbosity(LogLevelNum::Warn) // Cli.verbose Warn Info
        .init()
        .unwrap();

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
    unsafe {
        set_repository(repo);
    }

    let repo = get_repository();
    let head = repo.head();

    // load one Segment:
    let mut root = GitHierarchy::Name("mmc-fixes".to_string());
    println!("root is {}", root.node_identity());
    graph::discover::discover_graph(vec!(Box::new(root)) );

    // let msg = repo.message();
    // println!("{:?}", &head);

    unsafe {
        unset_repository();
    }
}
