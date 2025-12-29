#![allow(static_mut_refs)]
#![deny(elided_lifetimes_in_paths)]

use crate::utils::concatenate;
use git2::{Branch, Commit, Oid,
           Error,
           Reference, Repository, build::CheckoutBuilder,
           StatusShow,StatusOptions, Statuses,};
use std::cell::OnceCell;
#[allow(unused)]
use tracing::{debug, info, warn};

// owned git.

// or `Cell' get() will clone.  Repository can be cloned? `No'!
// take() replace() into_inner  set()
/*
    = note: the following trait bounds were not satisfied:
            `git2::Repository: Copy`
            which is required by `Option<git2::Repository>: Copy`
*/

// cannot be cloned, so we need reference.
// OnceCell

//  `RefCell' ... dynamic borrowing ....
static mut GLOBAL_REPOSITORY: OnceCell<Repository> = OnceCell::new();

// or
// static mut GLOBAL_REPOSITORY : Option<RefCell<Repository>> = None;

// how come?
// why reference? it's 1 pointer inside anyway!
/// we guarantee, that no change happens while a function has a reference.
/// i.e. once shared references are given out, no .... this is a refcell!
pub fn get_repository() -> &'static Repository {
    let &mut repository; // : &Repository;

    unsafe {
        // RefCell ...
        // as_ref()      &Option<T>` to `Option<&T>
        // borrow_mut()
        // assert GLOBAL_REPOSITORY.get().is_none();
        repository = GLOBAL_REPOSITORY.get();
        // .as_ref().expect("no repository"); // unwrap custom msg?
    };
    repository.expect("no repository")
}

// consumes, so moves?
// todo: drop this
pub fn set_repository(repo: Repository) {
    unsafe {
        let _ = GLOBAL_REPOSITORY
            .set(repo)
            .map_err(|_e| { panic!() });
    }
}
pub fn unset_repository() {
    // borrow_mut or replace
    unsafe {
        GLOBAL_REPOSITORY.take();
    }
}

// this consults the store.
pub fn git_same_ref(
    repository: &Repository,
    reference: &Reference<'_>,
    next: &Reference<'_>,
) -> bool {
    fn sha<'a>(_repository: &'a Repository, reference: &Reference<'a>) -> Oid {
        let direct = reference.resolve().unwrap(); // symbolic -> direct
        let oid = direct.target().unwrap();
        debug!("git_same_ref: {:?} {:?}",
               reference.name().unwrap(),
               oid);
        oid
    }

    sha(repository, reference) == sha(repository, next)
}

pub const GIT_HEADS_PATTERN: &str = "refs/heads/";

/// alternative to:
/// git checkout name -b target
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


pub fn staged_files<'repo>(repository: &'repo Repository) -> Result<Statuses<'repo>, Error>{
    let mut status_options = StatusOptions::new();
    status_options
        .show(StatusShow::Index)
        .include_unmodified(false) ;

    repository.statuses(Some(&mut status_options))
}

//Todo: I need my error.
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

pub fn force_head_to(repository: &Repository, name: &str, new_head: &Reference<'_>) {
    let oid = new_head.peel_to_commit().unwrap();
    // create it:
    debug!("relocating {:?} to {:?}", name, oid);
    repository.branch(name, &oid, true).unwrap();
    // git_run(repository, &["branch", "--force", segment.name(), new_head.name().unwrap()]);

    // checkout, since then I drop ...:
    let full_name = concatenate("refs/heads/", name);
    repository.set_head(&full_name).expect("failed to checkout");
    // git_run(repository, &["checkout", "--no-track", "-B", segment.name()]);
}

