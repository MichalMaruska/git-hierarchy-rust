// can I put this into ../Cargo.toml
#![deny(elided_lifetimes_in_paths)]

#[allow(unused)]
use tracing::{info,warn,debug};

use crate::base::{git_same_ref,GIT_HEADS_PATTERN,get_repository};

use std::cell::RefCell;

use crate::graph::discover::NodeExpander;

use crate::utils::{concatenate,extract_name};

use std::any::Any;
use git2::{Repository,Reference,Commit};

// low level sum & segment
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

    debug!("searching for sum {}",  name);
    if let Ok(ref_iterator) = repository.references_glob (&(concatenate(SUM_SUMMAND_PATTERN, name) + "/*")) {
        for r in ref_iterator {
            v.push(r.unwrap());
        }}

    return v;
}

fn branch_name<'a,'repo>(reference: &'a Reference<'repo>) -> &'a str {
    return reference.name().unwrap().strip_prefix(GIT_HEADS_PATTERN).unwrap();
}

///
pub struct Segment<'repo> {
    name: String,
    pub reference: RefCell<Reference<'repo>>,

    base: Reference<'repo>,
    pub _start: Reference<'repo>,
}

const REBASED_REFLOG :&str = "Rebased";

impl<'repo> Segment<'repo> {

    pub fn new(reference: Reference<'repo>, base: Reference<'repo>, start: Reference<'repo>) -> Segment<'repo> {
        Segment::<'repo> {
            name: branch_name(&reference).to_owned(),
            reference: RefCell::new(reference),
            base,
            _start: start
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn uptodate(&self, repository: &Repository) -> bool {
        // debug!("looking at segment: {:?} {:?}", self.base.name().unwrap(), self._start.name().unwrap());
        git_same_ref(repository, &self.base, &self._start)
    }

    pub fn empty(&self, repository: &Repository) -> bool {
        git_same_ref(repository, &self.reference.borrow(), &self._start)
    }

    pub fn git_revisions(&self) -> String {
        format!("{}..{}",
                self._start.name().unwrap(),
                self.reference.borrow().name().unwrap())
    }

    pub fn reset(&self, repository: &'repo Repository) {
        // re-resolve:
        let name = self.reference.borrow();
        // we cannot extract other references from there.
        self.reference.replace(repository.find_reference(name.name().unwrap()).unwrap());

        let mut start_ref = repository.find_reference(self._start.name().unwrap()).unwrap();
        let oid = self.base(&repository).target_peel().unwrap();
        warn!("setting {} to {}", start_ref.name().unwrap(), oid);
        if start_ref.set_target(oid, REBASED_REFLOG).is_err() {
            panic!("failed to set start to new base")
        }
    }

    pub fn base(&self, repository: &'repo Repository) -> Reference<'repo> {
        let reference = repository.find_reference(self.base.symbolic_target()
            .expect("base should be a symbolic reference")).unwrap();
        debug!("segment|base points at {:?}", reference.name().unwrap());
        return reference;
    }
}

pub struct Sum<'repo> {
    pub reference: Reference<'repo>,
    pub summands: Vec<Reference<'repo>>,
}

impl<'repo>  Sum<'repo> {

    // vec to vec
    pub fn summands(&self, repository: &'repo Repository) -> Vec<GitHierarchy<'repo>> {

        let mut resolved : Vec<GitHierarchy<'repo>> = Vec::with_capacity(self.summands.len());

        for summand in &self.summands {
            let symbolic_base = repository.find_reference(summand.symbolic_target().
                                                          expect("base should be a symbolic reference")).unwrap();
            resolved.push(GitHierarchy::Name(
                symbolic_base.name().unwrap().to_string()
            ))
        }
        return resolved;
    }


    pub fn name(&self) -> &str {
        // fixme: same as ....
        return branch_name(&self.reference);
    }
}

pub enum GitHierarchy<'repo> {
    Name(String),

    Segment(Segment<'repo>),
    Sum(Sum<'repo>),

    Reference(Reference<'repo>),
}

impl<'repo> GitHierarchy<'repo> {

    pub fn commit(&self) -> Commit<'_> {
        let reference: &Reference<'_> =
            match &self {
                GitHierarchy::Name(x) => {
                    eprintln!("trying {x}");
                    panic!("bad state");
                    // unimplemented!(),
                }
                GitHierarchy::Segment(s) => &s.reference.borrow(),
                GitHierarchy::Sum(s) => &s.reference,
                GitHierarchy::Reference(r) => &r
            };
        return reference.peel_to_commit().unwrap();
    }
}

//  Vertex -> 1st stage children       ..... looked up if already in the graph/queue.
//            1st stage ----(convert)---> Vertices.
// Given GH::Name,
// spreadsheet  Cell -> Formula & references.
pub fn load<'repo>(repository: &'repo Repository, name: &'_ str) -> Result<GitHierarchy<'repo>, git2::Error> {

    let name = extract_name(name);
    let reference = repository.resolve_reference_from_short_name(name)?;

    if let Ok(base) =  repository.find_reference(base_name(name).as_str()) {
        if let Ok(start) = repository.find_reference(start_name(name).as_str()) {

            info!("segment found {}", name);
            return Ok(GitHierarchy::Segment(Segment::new(reference, base, start)));
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

    info!("plain reference {}", name);
    return Ok(GitHierarchy::Reference(reference));
}

// note: trait items always share the visibility of their trait
impl<'a> crate::graph::discover::NodeExpander for GitHierarchy<'a> {
    fn node_identity(&self) -> &str {
        match self {
            Self::Name(x) => x,
            GitHierarchy::Segment(s) => s.name(),
            GitHierarchy::Sum(s) => s.name(),
            GitHierarchy::Reference(r) => r.name().unwrap(),
        }
    }

    // we need a repository!
    fn node_prepare(&mut self) {
        info!("prepare {:?}", self.node_identity());
        match self {
            Self::Name(x) => {
                let repository = get_repository();
                if let Ok(c) = load(repository, x) {
                    // c is GitHierarchy<'static> here I move
                    *self = c;
                }
            }
            // all these... a bug? Nothing to do:
            Self::Segment(_s) => {}
            Self::Sum(_s) => {}
            Self::Reference(_r) => {
                info!("Reference!");
            }
        }
    }

    // just get the Names.
    fn node_children(&self) -> Vec<Box<dyn NodeExpander +'_>>
    {
        let repository = get_repository();
        match self {
            // regular branch. say `master'
            Self::Name(_x) => {panic!("unprepared")}
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
            }
        }
    }
}
