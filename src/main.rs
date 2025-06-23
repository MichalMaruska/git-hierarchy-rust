
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

struct Segment<'repo> {
    reference: Reference<'repo>,
    base: Reference<'repo>,
    start: Reference<'repo>,
}


struct Sum<'repo> {
    reference: Reference<'repo>,
    summands: Vec<Reference<'repo>>,
}

fn concatenate(prefix: &str, suffix: &str) -> String {
    let mut s = String::from(prefix);
    s.push_str(suffix);
    s
}

const SEGMENT_BASE_PATTERN : &str = "refs/base/";
const SEGMENT_START_PATTERN : &str = "refs/start/";
const SUM_SUMMAND_PATTERN : &str = "refs/sums/";

fn base_name(name: &str) -> String {
    concatenate(SEGMENT_BASE_PATTERN, name)
}

fn start_name(name: &str) -> String {
        concatenate(SEGMENT_START_PATTERN, name)
}

fn sum_summands<'repo>(repository: &'repo Repository, name: &str) -> Vec<Reference<'repo>> {
    let mut v = Vec::new();

    if let Ok(ref_iterator) = repository.references_glob (&(concatenate(SUM_SUMMAND_PATTERN, name) + "/*")) {
        for r in ref_iterator {
            v.push(r.unwrap());
        }}

    return v;
}

enum GitHierarchy<'repo> {
    Name(String),

    Segment(Segment<'repo>),
    Sum(Sum<'repo>),

    Reference(Reference<'repo>),
}


// static
static mut GLOBAL_REPOSITORY : Option<Repository> = None;


fn get_repository() -> &'static Repository {

    let mut repository : & Repository;

    unsafe {
        repository = GLOBAL_REPOSITORY.as_ref().expect("no repository"); // unwrap custom msg?
    };
    return repository;
}


fn main() {
    let cli = Cli::parse();

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
    unsafe {
        GLOBAL_REPOSITORY = Some(repo);
    }

    let repo = get_repository();
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

    unsafe {
        GLOBAL_REPOSITORY = None;
    }
}
