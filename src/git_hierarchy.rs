use log::info;

use crate::base::*;
use std::any::Any;

use crate::graph::discover::NodeExpander;

use crate::utils::{concatenate,extract_name};
use git2::{Repository,Reference};


// low level sum & segment
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


///
pub struct Segment<'repo> {
    pub reference: Reference<'repo>, // this could point at GitHierarchy.
    base: Reference<'repo>, //&'repo mut GitHierarchy<'repo>,  //  Reference<'repo>
    pub _start: Reference<'repo>,
}

impl<'repo> Segment<'_> {
    pub fn name(&self) -> &str {
        self.reference.name().unwrap().strip_prefix(GIT_HEADS_PATTERN).unwrap()
    }

    pub fn reset(&self) {
        // fixme! &mut
        // set start to resolve(base)
    }
    // pub fn base(&self, repository: &Repository) -> Reference {
    // complains!!!
    pub fn base(&self, repository: &'repo Repository) -> Reference<'repo> {
        let step = repository.find_reference(self.base.symbolic_target()
            .expect("base should be a symbolic reference")).unwrap();

        return step;
    }
}

pub struct Sum<'repo> {
    reference: Reference<'repo>,
    summands: Vec<Reference<'repo>>,
}


pub enum GitHierarchy<'repo> {
    Name(String),

    Segment(Segment<'repo>),
    Sum(Sum<'repo>),

    Reference(Reference<'repo>),
}


fn convert<'a>(name: &'a str) -> Result<GitHierarchy<'static>, git2::Error> {

    let repository = get_repository();

    let name = extract_name(name);
    let reference = repository.find_reference(&concatenate(GIT_HEADS_PATTERN, name))?;

    if let Ok(base) =  repository.find_reference(base_name(name).as_str()) {
        if let Ok(start) = repository.find_reference(start_name(name).as_str()) {

            // event!(Level::INFO, "segment found!");
            info!("segment found");

            return Ok(GitHierarchy::Segment( Segment {
                reference: reference,
                base,
                // so it's a name, not Reference, not GitHierarchy !? but it could be
                _start: start
            }));
        } else { return Err(git2::Error::from_str("start not found")) };
    }

    let summands = sum_summands(&repository, name);
    if ! summands.is_empty() {
        info!("a sum detected {}", name);
        return Ok(GitHierarchy::Sum(Sum {
            reference: reference,
            summands
        }));
    }

    info!("plain reference");
    return Ok(GitHierarchy::Reference(reference));
}

// note: trait items always share the visibility of their trait
impl<'a : 'static> crate::graph::discover::NodeExpander for GitHierarchy<'a> {

    fn node_identity(&self) -> &str {
        match self {
            Self::Name(x) => x,
            GitHierarchy::Segment(s) => s.name(),
            GitHierarchy::Sum(s) => s.reference.name().unwrap(),
            GitHierarchy::Reference(r) => r.name().unwrap(),
        }
    }

    fn as_any(& self) -> &dyn Any {
        self
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
            Self::Segment(_s) => {}
            Self::Sum(_s) => {}
            Self::Reference(_r) => {
                info!("Reference!"); // bug!
            } // are you sure?
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
            Self::Name(_x) => {panic!("unprepared")}// {Vec::new()}
            Self::Segment(s) => {
                let symbolic_base = s.base(&repository);
                // back to name...
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
            Self::Reference(_r) => {
                Vec::new()
            } // are you sure?
            //
            // GitHierarchy::segment(s) => s.name,
            // GitHierarchy::sum(s) => s.name,
        }
    }
}
