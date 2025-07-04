#![allow(static_mut_refs)]

use std::cell::{OnceCell};
use git2::{Repository,Reference};

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

// consumes!
pub fn set_repository(repo: Repository) {
    // borrow_mut or replace
    unsafe {
        let Ok(_v) = GLOBAL_REPOSITORY.set(repo) else { panic!()};
    }
}
pub fn unset_repository() {
    // borrow_mut or replace
    unsafe {
        GLOBAL_REPOSITORY.take();
    }
}

pub fn git_same_ref(repository: &Repository, reference: &Reference, next: &Reference) -> bool {
    let commit1 = repository.find_commit(reference.target().unwrap()).unwrap();
    let commit2 = repository.find_commit(next.target().unwrap()).unwrap();
    commit1.id() == commit2.id()
}
