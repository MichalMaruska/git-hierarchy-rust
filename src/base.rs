#![allow(static_mut_refs)]
#![deny(elided_lifetimes_in_paths)]

use crate::utils::concatenate;
use git2::{Branch, Commit, Oid,
           Error,
           Reference, Repository, build::CheckoutBuilder,
           Sort,
           StatusShow,StatusOptions, Statuses,};
#[allow(unused)]
use tracing::{debug, info, warn};

// this consults the store.
pub fn git_same_ref(
    repository: &Repository,
    reference: &Reference<'_>,
    next: &Reference<'_>,
) -> bool {
    fn sha<'a>(_repository: &'a Repository, reference: &Reference<'a>) -> Oid {
        let direct = reference.resolve().unwrap();
        let oid = direct.target().unwrap();
        debug!("git_same_ref: {:?} {:?}",
               reference.name().unwrap(),
               oid);
        oid
    }

    sha(repository, reference) == sha(repository, next)
}

// ancestor <---is parent-- ........ descendant
pub fn is_linear_ancestor(repository: &Repository, ancestor: Oid, descendant: Oid) -> Result<bool,git2::Error>
{
    if ancestor == descendant { return Ok(true);}

    let mut walk = repository.revwalk()?;
    walk.push(descendant)?; // .expect("should set upper bound for Walk");
    // segment.reference.borrow().target().unwrap()
    walk.hide(ancestor)?; // .expect("should set the lower bound for Walk");

    walk.set_sorting(Sort::TOPOLOGICAL)?; // .expect("should set the topo ordering of the Walk");

    if walk.next().is_none() {
        return Ok(false);
    }

    for oid in walk {
        if repository.find_commit(oid.unwrap())?.parent_count() > 1 {
            return Ok(false)
            // panic!("a merge found");
        }
    }
    Ok(true)
}

pub const GIT_HEADS_PATTERN: &str = "refs/heads/";

/// alternative to:
/// `git checkout name -b target`
/// git_run(repository, &["checkout", "--no-track", "-B", temp_head, new_start.name().unwrap()]);
pub fn checkout_new_head_at<'repo>(
    repository: &'repo Repository,
    name: Option<&'_ str>,
    target: &Commit<'_>,
) -> Option<Branch<'repo>> {
    // reflog?

    // https://libgit2.org/docs/reference/main/checkout/git_checkout_head.html
    // error: temporary value is freed at the end of this statement
    let tree = target.tree().unwrap();

    let mut checkout_opts = CheckoutBuilder::new();
    checkout_opts.safe();
    checkout_opts.force();

    repository
        .checkout_tree(tree.as_object(), Some(&mut checkout_opts))
        .expect("failed to checkout the newly created branch");

    if let Some(name) = name {
        info!("create temp branch {:?}", name);

        // target = target.peel_to_commit().unwrap()
        let new_branch = repository.branch(name, target, false).unwrap();

        let full_name = new_branch.name().unwrap().unwrap();
        let full_name = concatenate(GIT_HEADS_PATTERN, full_name);
        info!("checkout {:?} to {:?}", full_name, target);

        repository
            .set_head(&full_name)
            .expect("failed to create a branch on given commit");
        Some(new_branch)
    } else {
        info!("detached checkout {:?}", target.id());
        repository.set_head_detached(target.id()).unwrap();
        None
    }
}

// get the status: list of file modified in Index
pub fn staged_files<'repo>(repository: &'repo Repository) -> Result<Statuses<'repo>, Error>{
    let mut status_options = StatusOptions::new();
    status_options
        .show(StatusShow::Index)
        .include_unmodified(false) ;

    repository.statuses(Some(&mut status_options))
}

// Todo: I need my error.
// why not repository.state() == RepositoryState::Clean
pub fn repository_clean(repository: &Repository) -> bool {
    // rely on
    let options = &mut StatusOptions::new();
        options.include_untracked(false)
        .include_ignored(false);
    let statuses = repository.statuses(Some(options)).unwrap();
    if ! statuses.is_empty() {
        for entry in statuses.iter() {
            eprintln!("{:?}", entry.path());
        }
        return false;
    }
    true
}

// either a throw-away branch or fully determined ... sum.
// git_run(repository, &["branch", "--force", segment.name(), new_head.name().unwrap()]);
// git_run(repository, &["checkout", "--no-track", "-B", segment.name()]);
// fixme: why 2 things:
pub fn force_head_to(repository: &Repository, name: &str, new_head: &Reference<'_>) {
    let oid = new_head.peel_to_commit().unwrap();

    debug!("relocating {:?} to {:?}", name, oid);
    repository.branch(name, &oid, true).unwrap();

    // and `checkout'! Why?
    let full_name = concatenate(GIT_HEADS_PATTERN, name);
    repository.set_head(&full_name).expect("failed to checkout");
}
