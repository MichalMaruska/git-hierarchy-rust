// can I put this into ../Cargo.toml
#![deny(elided_lifetimes_in_paths)]

#[allow(unused)]
use tracing::{debug, info, warn};

use crate::base::{GIT_HEADS_PATTERN, git_same_ref};

use std::cell::RefCell;

use crate::graph::discover::NodeExpander;

use crate::utils::{concatenate, extract_name};

use git2::{Commit, Oid, Reference, Repository, Revwalk, Sort, Error};

// low level sum & segment
const SEGMENT_BASE_PATTERN: &str = "refs/base/";
const SEGMENT_START_PATTERN: &str = "refs/start/";
const SUM_SUMMAND_PATTERN: &str = "refs/sums/";
const SEPARATOR : &str = "/";

fn base_name(name: &str) -> String {
    concatenate(SEGMENT_BASE_PATTERN, name)
}

fn start_name(name: &str) -> String {
    concatenate(SEGMENT_START_PATTERN, name)
}

fn sum_summands<'repo>(repository: &'repo Repository, name: &str) -> Vec<Reference<'repo>> {
    let mut v = Vec::new();

    debug!("searching for sum {}", name);
    if let Ok(ref_iterator) =
        repository.references_glob(&(concatenate(SUM_SUMMAND_PATTERN, name) + "/*"))
    {
        for r in ref_iterator {
            v.push(r.unwrap());
        }
    }

    return v;
}

fn branch_name<'a, 'repo>(reference: &'a Reference<'repo>) -> &'a str {
    return reference
        .name()
        .unwrap()
        .strip_prefix(GIT_HEADS_PATTERN)
        .unwrap();
}

///
pub struct Segment<'repo> {
    name: String,
    pub reference: RefCell<Reference<'repo>>,

    pub base: RefCell<Reference<'repo>>, // I need to call &mut methods
    pub _start: Reference<'repo>,
}

const REBASED_REFLOG: &str = "Rebased";

impl<'repo> Segment<'repo> {
    pub fn create(repository: &'repo Repository,
                  name: &str,
                  // why the same?
                  base: &'_ Reference<'_>,
                  start: &'_ Reference<'_>,
                  head: &'_ Reference<'_>)
                         -> Result<Segment<'repo>, Error> {
        info!("create segment: {}", name);
        let h = repository.branch(name, &head.peel_to_commit().unwrap(), false)?; // .expect("should be a new reference");
        let s = repository.reference(&concatenate(SEGMENT_START_PATTERN, name),
                                     start.target().unwrap(),
                                     false,
                                     "start").expect("should be a new reference");
        let b = repository.reference_symbolic(&concatenate(SEGMENT_BASE_PATTERN, name),
                                              base.name().unwrap(),
                                              false,
                                              "new segment").expect("should be a new symbolic reference");
        // unimplemented!();
        // I need to own the reference:
        Ok(Segment::new(h.into_reference(), b, s))
    }

    pub fn new(
        reference: Reference<'repo>,
        base: Reference<'repo>,
        start: Reference<'repo>,
    ) -> Segment<'repo> {
        Segment::<'repo> {
            name: branch_name(&reference).to_owned(),
            reference: RefCell::new(reference),
            base: RefCell::new(base),
            _start: start,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn uptodate(&self, _repository: &Repository) -> bool {
        // debug!("looking at segment: {:?} {:?}", self.base.name().unwrap(), self._start.name().unwrap());
        self.base
            .borrow().peel_to_commit().unwrap().id() == self._start.target().unwrap()
    }

    pub fn empty(&self, repository: &Repository) -> bool {
        git_same_ref(repository, &self.reference.borrow(), &self._start)
    }

    pub fn git_revisions(&self) -> String {
        format!(
            "{}..{}",
            self._start.name().unwrap(),
            self.reference.borrow().name().unwrap()
        )
    }

    pub fn reset(&self, repository: &'repo Repository, oid: Oid) {

        let head_reference = self.reference.borrow();

        // I want to refresh this!
        debug!("reset: the head itself? {} with {}",
               head_reference.name().unwrap(),
               oid
        );
        drop(head_reference);
        // we cannot extract other references from there.
        self.reference.replace_with(|r| r.set_target(oid, "rebased").unwrap());

        // fixme: what? ref -> name -> ref? b/c &self is not &mut?
        let start_ref_name = self._start.name().unwrap();
        let mut start_ref = repository.find_reference(start_ref_name).unwrap();

        let base = self.base(repository);
        debug!("base to {:?}", base.target());
        // _peel fails!
        let oid = base.target().unwrap();
        // debug!("reset: {} to {}", self.name(), oid);
        warn!("setting {} to {}", start_ref_name, oid);
        if start_ref.set_target(oid, REBASED_REFLOG).is_err() {
            panic!("failed to set start to new base")
        }
    }

    pub fn base(&self, repository: &'repo Repository) -> Reference<'repo> {
        let reference = repository
            .find_reference(
                self.base.borrow()
                    .symbolic_target()
                    .expect("base should be a symbolic reference"),
            )
            .unwrap();
        debug!("base points at {:?}", reference.name().unwrap());
        return reference;
    }


    pub fn iter(&self, repository: &'repo Repository) -> Result<Revwalk<'_>, Error> {

        let mut walk = repository.revwalk()?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
        // target_peel fails!
        let oid = self.reference.borrow().target().unwrap();
        walk.push(oid)?;
        //_peel
        let oid = self._start.target().unwrap();
        walk.hide(oid)?;

        return Ok(walk);
    }
}



pub struct Sum<'repo> {
    name: String,
    pub reference: RefCell<Reference<'repo>>,
    summands: Vec<Reference<'repo>>,
    // resolved: RefCell<Option<Vec<GitHierarchy<'repo>>>>,
}

impl<'repo> Sum<'repo> {


    pub fn new(
        reference: Reference<'repo>,
        summands: Vec<Reference<'repo>>
        // base: Reference<'repo>,
        // start: Reference<'repo>,
    ) -> Sum<'repo> {
        Sum::<'repo> {
            name: branch_name(&reference).to_owned(),
            reference: RefCell::new(reference),
            summands: summands,
        }
    }

    pub fn create<'a>(
        repository: &'repo Repository,
        name: &str,
        components: impl Iterator<Item = &'a Reference<'repo>>,
        // I need mut to take ownership if items.
        hint: Option<Commit<'repo>>) -> Result<Sum<'repo>, Error>
        where 'repo : 'a
    {
        info!("create sum: {}", name);
        // let zipper: Vec<_> = (0..).zip("foo".chars()).collect();

        // create  sum/1 ... sum/N symbolic references.
        let summands : Vec<Reference<'repo>> = components.enumerate().map(
            |(n, s)|
            {
                repository.reference_symbolic(&(SUM_SUMMAND_PATTERN.to_string()
                                                + SEPARATOR
                                                + name
                                                + SEPARATOR
                                                + &n.to_string()),
                                              s.name().expect("should have name"),
                                              false,
                                              "start").expect("should be a new symbolic reference")
            }
        ).collect();

        // create branch
        let h = repository.branch(name,
                                  // either at hinted
                                  &hint.unwrap_or(summands[0].peel_to_commit().unwrap()),
                                  // &head.peel_to_commit().unwrap()
                                  false)?; // .expect("should be a new reference");


        return Ok(Self::new(h.into_reference(), summands));
    }


    // does this help? or better Vec<Reference>?
    pub fn summands(&self, repository: &'repo Repository) -> Vec<Reference<'repo>> {
        debug!("resolving summands for {:?}", self.name());
        let resolved: Vec<Reference<'repo>>;
        // = Vec::with_capacity(self.summands.len());

        resolved = self.summands.iter().map(
            |summand| {
                let symbolic_base = repository.find_reference(
                    summand
                        .symbolic_target()
                        .expect("base should be a symbolic reference"),
                ).unwrap();

                debug!("{:?} -> {:?}",
                       summand.name().unwrap(),
                       symbolic_base.name().unwrap()
                );

                // so a complete Ref name.
                // GitHierarchy::Name(symbolic_base.name().unwrap().to_string())
                symbolic_base
            }).collect();

        return resolved;
    }

    /*
    pub fn rewrite_summands(&self, value: Vec<&GitHierarchy<'repo>>) {
        // fixme: replace_with
        self.resolved.replace(Some(value));
    }
    */

    pub fn name(&self) -> &str {
        // fixme: same as ....
        return &self.name; // branch_name(&self.reference.borrow());
    }

    pub fn parent_commits(&self) -> Vec<Oid> {
        let commit = self.reference.borrow().peel_to_commit().unwrap();
        commit.parent_ids().collect()
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
        let reference: &Reference<'_> = match &self {
            GitHierarchy::Name(x) => {
                eprintln!("trying {x}");
                panic!("bad state");
                // unimplemented!(),
            }
            GitHierarchy::Segment(s) => &s.reference.borrow(),
            GitHierarchy::Sum(s) => &s.reference.borrow(),
            GitHierarchy::Reference(r) => &r,
        };
        return reference.peel_to_commit().unwrap();
    }
}

//  Vertex -> 1st stage children       ..... looked up if already in the graph/queue.
//            1st stage ----(convert)---> Vertices.
// Given GH::Name,
// spreadsheet  Cell -> Formula & references.
pub fn load<'repo>(
    repository: &'repo Repository,
    name: &'_ str,
) -> Result<GitHierarchy<'repo>, git2::Error> {
    let name = extract_name(name);
    let reference = repository.resolve_reference_from_short_name(name)?;

    if let Ok(base) = repository.find_reference(base_name(name).as_str()) {
        if let Ok(start) = repository.find_reference(start_name(name).as_str()) {
            info!("segment found {}", name);
            return Ok(GitHierarchy::Segment(Segment::new(reference, base, start)));
        } else {
            return Err(git2::Error::from_str("start not found"));
        };
    }

    let summands = sum_summands(&repository, name);
    if !summands.is_empty() {
        info!("a sum detected {}", name);
        return Ok(GitHierarchy::Sum(Sum::new(reference, summands)))};

    info!("plain reference {}", name);
    return Ok(GitHierarchy::Reference(reference));
}

// note: trait items always share the visibility of their trait
impl<'a> NodeExpander for GitHierarchy<'a> {
    fn node_identity(&self) -> &str {
        match self {
            Self::Name(x) => x,
            GitHierarchy::Segment(s) => s.name(),
            GitHierarchy::Sum(s) => s.name(),
            GitHierarchy::Reference(r) => r.name().unwrap(),
        }
    }
}
