#![allow(unused)]
// deny, warn, allow...
#![warn(static_mut_refs)]
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

use std::collections::HashMap;
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
    fn node_identity(&self) -> &str; // same lifetime

    // not object-safe:
    // so Self is not ok, but NodeExpander is ?
    fn node_prepare(&mut self); // -> &dyn NodeExpander; // upgrade itself? or what
    fn node_children(&self) -> Vec<Box<dyn NodeExpander>>; // owned!
}


pub fn discover_graph(mut start: Vec<Box<dyn NodeExpander>>) // expander: &dyn NodeExpander) { // -> (vertices, graph) {
{
    let mut graph = graph::Graph::new();
    graph.add_vertices(start.len());

    // not mutable. but internal mutability!

    let mut vertices : Vec<Box<dyn NodeExpander>> = Vec::new();
    // |start|.....
    // |------|-------------|......|  vertices
    //        ^ reader      ^appender
    //
    vertices.append(&mut start);  // _from_slice(start);
    /*
    vertices.reserve(start.len());
    start.into_iter().map(|x| vertices.push(x)); // move
     */
    // I give up: &str
    let mut known : HashMap<String, usize> = HashMap::new(); // knowledge  name -> index

    //
    let mut current = 0;
    loop {
        let this = vertices.get_mut(current).unwrap();

        info!("visiting node {} {}", current, this.node_identity());
        this.node_prepare();
        let children =  this.node_children();
        for child in children {
            if let Some(found) = known.get(child.node_identity()) {
                info!("adding edge to already known node {}", child.node_identity());
                graph.add_edge(current, *found);
            } else {
                vertices.push(child);
                let new_index = vertices.len() - 1;
                graph.add_vertices(new_index);
                graph.add_edge(current, new_index);
                info!("adding unknown child to the list {} {}", &vertices[new_index].node_identity(), new_index);

                known.insert(vertices[new_index].node_identity().to_string(), new_index);
            }
        }

        current+= 1;
        if current == vertices.len() {break}
    }
    graph.dump_graph();
}

struct Segment<'repo> {
    reference: Reference<'repo>, // this could point at GitHierarchy.
    base: Reference<'repo>, //&'repo mut GitHierarchy<'repo>,  //  Reference<'repo>
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
const GIT_HEADS_PATTERN : &str = "refs/heads/";

fn base_name(name: &str) -> String {
    concatenate(SEGMENT_BASE_PATTERN, name)
}

fn start_name(name: &str) -> String {
        concatenate(SEGMENT_START_PATTERN, name)
}

fn sum_summands<'repo>(repository: &'repo Repository, name: &str) -> Vec<Reference<'repo>> {
    let mut v = Vec::new();

    info!("searching for sum {}",  name);
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


fn extract_name(refname: &str) -> &str {
    let mut a = refname.strip_prefix("ref: ").unwrap_or(refname);
    a = a.strip_prefix("refs/").unwrap_or(a);
    a = a.strip_prefix("heads/").unwrap_or(a);
    return a;
}

fn convert<'a>(name: &'a str) -> Result<GitHierarchy<'static>, git2::Error> {

    let repository = get_repository();

    let name = extract_name(name);
    println!("find reference {name}");
    let reference = repository.find_reference(&concatenate(GIT_HEADS_PATTERN, name))?;

    if let Ok(base) =  repository.find_reference(base_name(name).as_str()) {
        if let Ok(start) = repository.find_reference(start_name(name).as_str()) {

            // event!(Level::INFO, "segment found!");
            info!("segment found");

            return Ok(GitHierarchy::Segment( Segment {
                reference: reference,
                base,
                // so it's a name, not Reference, not GitHierarchy !? but it could be
                start: start
            }));
        } else { return Err(git2::Error::from_str("start not found")) };
    }

    let summands = sum_summands(repository, name);
    if ! summands.is_empty() {
        info!("a sum detected {}", name);
        return Ok(GitHierarchy::Sum(Sum {
            reference: reference,
            summands
        }));
    }

    info!("plain reference");
    return Err(git2::Error::from_str("not hierarchy"));
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



impl<'a> NodeExpander for GitHierarchy<'a> {

    fn node_identity(&self) -> &str {
        match self {
            Self::Name(x) => x,
            GitHierarchy::Segment(s) => s.reference.name().unwrap(),
            GitHierarchy::Sum(s) => s.reference.name().unwrap(),
            GitHierarchy::Reference(r) => r.name().unwrap(),
        }
    }

    // we need a repository!
    fn node_prepare(&mut self) { //  -> &str {   '1 lifetime
        info!("prepare {:?}", self.node_identity());
        match self {
            Self::Name(x) => {
                if let Ok(c) = convert(x) {
                    // match c {
                    //    Segment(s) =>
                    // lifetime? who keeps c up? .... so I need a Vec of Segments?
                    // or Rc .... a hashMap.
                    *self = c; // .Segment =
                    // return self
                }
                // } else { panic!("missing repo");}
            }
            Self::Segment(s) => {}
            Self::Sum(s) => {}
            Self::Reference(r) => {} // are you sure?
            //
            // GitHierarchy::segment(s) => s.name,
            // GitHierarchy::sum(s) => s.name,
        }
    }

    fn node_children(&self) -> Vec<Box<dyn NodeExpander>> // array?
    {
        // just get the Names.
        let repository = get_repository();
        match self {
            // regular branch. say `master'
            Self::Name(x) => {Vec::new()}
            Self::Segment(s) => {
                let symbolic_base = repository.find_reference(s.base.symbolic_target().
                    expect("base should be a symbolic reference")).unwrap();
                vec!( Box::new(GitHierarchy::Name(symbolic_base.name().unwrap().to_string())))
            }
            Self::Sum(s) => {
                // copy
                let mut v : Vec<Box<dyn NodeExpander>> = Vec::new();
                for summand in &s.summands {
                    let symbolic_base = repository.find_reference(summand.symbolic_target().
                        expect("base should be a symbolic reference")).unwrap();
                    v.push(Box::new(GitHierarchy::Name(
                        symbolic_base.name().unwrap().to_string())))
                }
                return v;
            }
            Self::Reference(r) => {
                Vec::new()
            } // are you sure?
            //
            // GitHierarchy::segment(s) => s.name,
            // GitHierarchy::sum(s) => s.name,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    stderrlog::new().module(module_path!())
        .verbosity(LogLevelNum::Info) // Cli.verbose Warn
        .init()
        .unwrap();

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
    // println!("{:?}", repo.namespace());
    let head = repo.head();

    // load one Segment:
    let mut root = GitHierarchy::Name("mmc-fixes".to_string());
    println!("root is {}", root.node_identity());
    discover_graph(vec!(Box::new(root)) );

    // let msg = repo.message();
    // println!("{:?}", &head);

    unsafe {
        GLOBAL_REPOSITORY = None;
    }
}
