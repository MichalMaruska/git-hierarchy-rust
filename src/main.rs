
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

//
// get list of segments
use git2::{Repository,Reference,Error};
use clap::Parser;
// use std::error::Error;

// This declaration will look for a file named `graph'.rs and will
// insert its contents inside a module named `my` under this scope
mod graph;

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

pub trait NodeExpander {
    fn NodeIdentity(&self) -> &str; // same lifetime

    // not object-safe:
    // so Self is not ok, but NodeExpander is ?
    fn NodePrepare(&self) -> &dyn NodeExpander; // upgrade itself? or what
    fn NodeChildren(&self) -> [&dyn NodeExpander]; // owned!
}



fn main() {
    let cli = Cli::parse();

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
    // `git2::Repository` cannot be formatted with the default formatter
    // `git2::Repository` cannot be formatted using `{:?}` because it doesn't implement `std::fmt::Debug`
    println!("{:?}", repo.namespace());
    let head = repo.head();

    if let Ok(refsi) = repo.references_glob (&GLOB_REFS_BASES) {
        for reference in refsi {
            println!("{:?}", reference.unwrap().name());
        }
    }



    // let msg = repo.message();
    // println!("{:?}", &head);
}
