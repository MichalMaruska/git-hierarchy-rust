use git_hierarchy::git_hierarchy::Segment;
use std::env;
use git2::Repository;

fn main() {
    let args: Vec<String> = env::args().collect();
    let name = &args[1];
    let reference_name = &args[2];

    let repo = Repository::open_from_env().unwrap();
    let reference = repo.find_reference(reference_name).unwrap();

    Segment::create(&repo, name, &reference, &reference, &reference).unwrap();
}