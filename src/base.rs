#![allow(static_mut_refs)]
#![deny(elided_lifetimes_in_paths)]

use std::cell::{OnceCell};
use git2::{Repository,Reference,Commit,Branch};
use crate::utils::{concatenate};
#[allow(unused)]
use tracing::{warn,info,debug};

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
static mut GLOBAL_REPOSITORY : OnceCell<Repository> = OnceCell::new();

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
    return repository.expect("no repository");
}

// consumes, so moves?
pub fn set_repository(repo: Repository) {
    unsafe {
        let _ = GLOBAL_REPOSITORY.set(repo).or_else(|_e| -> Result<(), ()> {panic!()});
    }
}
pub fn unset_repository() {
    // borrow_mut or replace
    unsafe {
        GLOBAL_REPOSITORY.take();
    }
}

// this consults the store.
pub fn git_same_ref(repository: &Repository, reference: &Reference<'_>, next: &Reference<'_>) -> bool {

    fn sha<'a>(repository: &'a Repository, reference: &Reference<'a>) -> Commit<'a> {
        let direct = reference.resolve().unwrap();
        debug!("git_same_ref: {:?} {:?}", reference.name().unwrap(), direct.target());
        repository.find_commit(direct.target().unwrap()).unwrap()
    }

    sha(repository, reference).id() == sha(repository, next).id()
}

pub const GIT_HEADS_PATTERN : &str = "refs/heads/";

/// alternative to:
/// git checkout name -b target
/// git_run(repository, &["checkout", "--no-track", "-B", temp_head, new_start.name().unwrap()]);
pub fn checkout_new_head_at<'repo>(repository: &'repo Repository,
                                   name: &'_ str,
                                   target: &Commit<'_>) -> Branch<'repo> {
    // reflog?
    info!("create temp branch {:?}", name);

    // target = target.peel_to_commit().unwrap()
    let new_branch = repository.branch(name, target, false).unwrap();

    let full_name = new_branch.name().unwrap().unwrap();
    let full_name = concatenate(GIT_HEADS_PATTERN,  full_name);
    info!("checkout {:?} to {:?}", full_name, target);

    // https://libgit2.org/docs/reference/main/checkout/git_checkout_head.html
    // error: temporary value is freed at the end of this statement
    let tree = target.tree().unwrap();
    repository.checkout_tree(tree.as_object(), None).expect("failed to checkout the newly created branch");

    repository.set_head(&full_name).expect("failed to create a branch on given commit");
    return new_branch;
}
