#![deny(elided_lifetimes_in_paths)]
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

use clap::Parser;
use git2::{Repository,Reference,Error,Branch,BranchType,ReferenceFormat};

use tracing::{warn,info,debug};

use ::git_hierarchy::base::{get_repository,set_repository,unset_repository,git_same_ref,checkout_new_head_at};
use ::git_hierarchy::utils::{extract_name,divide_str,concatenate};
use ::git_hierarchy::execute::git_run;

use crate::graph::discover_pet::find_hierarchy;

// I need both:
#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy,Segment,Sum};

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph;
use graph::discover::NodeExpander;


enum RebaseResult {
    Nothing,
    Done,
    // Failed,
}

const TEMP_HEAD_NAME : &str = "tempSegment";


fn create_marker_file() {
    // persistent mark, if we fail, and during the session.
    /*
    mark := plumbing.NewSymbolicReference(".segment-cherry-pick", segment.Ref.Name());
    err := repository.Storer.SetReference(mark)
    */
}

// either exit or rewrite the segment ....its reference should update oid.
fn rebase_segment(repository: &Repository, segment: &Segment<'_>) -> RebaseResult {
    info!("should rebase {}", segment.name());

    if segment.uptodate(repository) {
        info!("nothing to do -- base and start equal");
        return RebaseResult::Nothing;
    }

    let new_start = segment.base(repository);

    // todo: segment_empty()
    if segment.empty(repository)  {
        return rebase_empty_segment(segment, repository);
    }

    debug!("rebasing by Cherry-picking {}!", segment.name());
    // can I raii ? so drop() would remove the file?
    create_marker_file();

    // checkout to that ref
    // todo: git stash
    // must change to the directory!
    let temp_head = TEMP_HEAD_NAME;
    Branch::name_is_valid(temp_head).unwrap();
    let temp_head = checkout_new_head_at(repository, temp_head,
                            &new_start.peel_to_commit().unwrap());

    if !git_run(repository, &["cherry-pick", segment.git_revisions().as_str() ]).success() {
        // return RebaseResult::Failed;
        panic!("cherry-pick failed");
    }

    // I have to re-find it?
    rebase_segment_finish(repository, segment,
                                       // temp_head.get()
                                       repository.find_branch(&TEMP_HEAD_NAME, BranchType::Local).unwrap().get());

    debug!("delete: {:?}", temp_head.name());
    if !git_run(repository, &["branch", "-D", temp_head.name().unwrap().unwrap() ]).success() {
        panic!("branch -D failed");
    }
    // temp_head.delete().expect("failed to delete a branch");
    return status;
}

fn rebase_empty_segment(segment: &Segment<'_>, repository: &Repository) -> RebaseResult {
    debug!("rebase empty segment: {}", segment.name());
    // fixme:  move Start to Base!
    segment.reset(repository);
    return RebaseResult::Done;
}

fn force_head_to(repository: &Repository, name: &str, new_head: &Reference<'_>) {
    debug!("relocating {:?} to {:?}", name, new_head.name().unwrap());
    let oid = new_head.peel_to_commit().unwrap();
    repository.branch(name, &oid, true);
    // git_run(repository, &["branch", "--force", segment.name(), new_head.name().unwrap()]);

    // checkout, since then I drop ...:
    let full_name = concatenate("refs/heads/",  name);
    repository.set_head(&full_name).expect("failed to checkout");
    // git_run(repository, &["checkout", "--no-track", "-B", segment.name()]);
}

fn rebase_segment_finish(repository: &Repository, segment: &Segment<'_>, new_head: &Reference<'_>) {
    segment.reset(repository);

    // reflog etc.
    force_head_to(repository, segment.name(), new_head);
}


fn fetch_upstream_of(repository: &Repository, reference: &Reference<'_>) -> Result<(), Error> {
    warn!("should fetch");
    // remote ->
    if reference.is_remote() {
        // mmc: I think it's dangerous ... better avoid using this.
        // let remote: RemoteHead;
        // just fetch
        // Remote.fetch()
        unimplemented!("Remote");
    } else if reference.is_branch() {
        let name = Reference::normalize_name(reference.name().unwrap(), ReferenceFormat::NORMAL).unwrap();
        warn!("fetch local {name}");
        let mut branch = repository.find_branch(extract_name(&name), BranchType::Local).unwrap();
        // let b = Branch::wrap(*reference); // cannot move out of `*reference` which is behind a mutable reference
        let upstream = branch.upstream().unwrap();
        // todo: double check if still in sync, then
        let upstream_name = upstream.name().unwrap().unwrap();

        // and this is host/branch
        // fixme:
        if git_same_ref(repository, reference, upstream.get()) {
            info!("in sync, so let's fetch & update");
        } else {
            warn!("NOT in sync; should not update.");
        }
        //
        let (rem, br) = divide_str(upstream_name, '/');
        let mut remote = repository.find_remote(rem)?;
        // repo.find_remote("origin")?.fetch(&["main"], None, None)
        if true {
            warn!("fetch {} {} ....", rem, br);
            if remote.fetch(&[br], None, None).is_ok() {
                let oid = branch.upstream().unwrap().get().target().expect("upstream disappeared");
                branch.get_mut().set_target(oid, "fetch & fast-forward").expect("fetch/sync failed");
            }
        }
        // sync the local
    }
    Ok(())
}


fn rebase_node<'repo>(repo: &Repository, node: &GitHierarchy<'_>, fetch: bool) {
    match node {
        GitHierarchy::Name(_n) => {panic!();}
        GitHierarchy::Reference(r) => {
            if fetch {
                fetch_upstream_of(repo, r).expect("fetch failed");
            }}
        GitHierarchy::Segment(segment)=> {
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(_sum) => {
            warn!("should re-merge");
        }
    }
}


fn start_rebase(repository: &Repository,
                root: String,
                fetch: bool) {

    let (object_map, // String -> GitHierarchy
         hash_to_graph,  // stable graph:  String -> index ?
         graph,          // index -> String?
         discovery_order) = find_hierarchy(repository, root);

    for v in discovery_order {
        println!("{:?} {:?} {:?}", v,
                 object_map.get(&v).unwrap().node_identity(),
                 graph.node_weight(
                     hash_to_graph.get(&v).unwrap().clone()).unwrap()
        );
        let vertex = object_map.get(&v).unwrap();
        rebase_node(repository, vertex, fetch);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short='g')]
    directory: Option<String>,
    #[arg(short, long)]
    fetch: bool,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    root_reference: Option<String>,
}

///
fn main() {

    let cli = Cli::parse();
    // cli can override the Env variable.
    if cli.verbose > 0 {
        tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();
    } else {
        tracing_subscriber::fmt::init();
    }

    let repo = match Repository::open(cli.directory.unwrap_or(".".to_string())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
    set_repository(repo);

    let repo = get_repository();

    // todo:
    // normalize_name(refname: &str, flags: ReferenceFormat) -> Result<String, Error> {
    // load one Segment:

    let root = cli.root_reference.unwrap_or_else(
        || repo.head().unwrap().name().unwrap().to_owned());
    let root = GitHierarchy::Name(root);

    println!("root is {}", root.node_identity());

    start_rebase(repo, root.node_identity().to_owned(),
                 cli.fetch);

    unset_repository();
}
