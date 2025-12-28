#![deny(elided_lifetimes_in_paths)]
// walk the hierarchy
// - assemble list of segments/sums.
// - graph, toposort

use clap::Parser;
#[allow(unused_imports)]
use git2::{Branch, BranchType, Error, Commit, Reference, ReferenceFormat, Repository,
           MergeOptions,
           build::CheckoutBuilder,
           Oid,
           RepositoryState,
           // merge:
           AnnotatedCommit,
           Sort,
};

#[allow(unused_imports)]
use tracing::{span, Level, debug, info, warn,error};

use ::git_hierarchy::base::{checkout_new_head_at, git_same_ref, force_head_to,};
use ::git_hierarchy::execute::git_run;
use ::git_hierarchy::utils::{
    divide_str, extract_name, find_non_matching_elements, init_tracing,
};
use ::git_hierarchy::rebase::{rebase_segment,rebase_segment_continue,RebaseResult};
use std::collections::HashMap;
use std::iter::Iterator;

use crate::graph::discover_pet::find_hierarchy;

// I need both:
#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy, Segment, Sum, load};

use std::path::PathBuf;
use std::process::exit;

/*
 note: ambiguous because of a conflict between a name from a glob import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph;
use graph::discover::NodeExpander;


/// Compose commit message for the Sum/Merge of .... components given by the
/// first/others.
fn get_merge_commit_message<'a, 'b, 'c, Iter>(
    sum_name: &'b str,
    first: &'c str,
    others: Iter,
) -> String
where
    Iter: Iterator<Item = &'a str>,
{
    let mut message = format!("Sum: {sum_name}\n\n{}", first);

    const NAMES_PER_LINE: usize = 3;
    for (i, name) in others.enumerate() {
        message.push_str(" + ");
        message.push_str(name);

        if i % NAMES_PER_LINE == 0 {
            // exactly same as push_str()
            message += "\n"
        }
    }
    message
}

/// Given @sum, check if it's up-to-date.
///
/// If not: create a new git merge commit.
fn remerge_sum<'repo>(
    repository: &'repo Repository,
    sum: &Sum<'repo>,
    object_map: &HashMap<String, GitHierarchy<'repo>>, // this lifetime
) -> RebaseResult {
    let summands = sum.summands(repository);

/* assumption:
    sum has its summands   base/1 ... base/N
    these might resolve to References. -- how is that different from Branch?

    During the rebasing we change ... Branches (References), and update them in the `object_map'
    so we .... prefer to look up there.
*/

    // find the representation which we already have and keep updating.
    let graphed_summands: Vec<&GitHierarchy<'_>> = summands
        .iter()
        .map(
            |s| {
                let gh = object_map.get(s.name().unwrap()).unwrap();
                debug!(
                    "convert {:?} to {:?}",
                    s.name().unwrap(),
                    gh.node_identity()
                );
                gh
            })
        .collect();

    // convert to the nodes?
    debug!("The current parent commits are: {:?}", sum.parent_commits());
    for c in sum.parent_commits() {
        debug!("  {}", c);
    }

    let v = find_non_matching_elements(
        // iter2 - hash(iter1)
        graphed_summands.iter(), // these are &GitHierarchy
        sum.parent_commits(),
        // we get reference.
        // sum.reference.peel_to_commit().unwrap().parent_ids().into_iter(),
        |gh| {
            debug!("mapping {:?} to {:?}", gh.node_identity(),
                   gh.commit().id());
            gh.commit().id()
        }, // I get:  ^^^^^^^^^^^ expected `Oid`, found `Commit<'_>`
    );

    if v.is_empty() {
        debug!("sum is update: summands & parent commits align");
    } else {
        info!("so the sum is not up-to-date!");

        let first = graphed_summands.first().unwrap();

        let message = get_merge_commit_message(
            sum.name(),
            first.node_identity(), // : &GitHierarchy
            graphed_summands.iter()
                .skip(1).map(|x| x.node_identity()),
        );

        if graphed_summands.len() > 2 {
            let temp_head = checkout_new_head_at(repository, Some("temp-sum"), &first.commit())
                .unwrap();

            // use  git_run or?
            let mut cmdline = vec![
                "merge",
                "-m",
                &message, // why is this not automatic?
                "--rerere-autoupdate",
                "--strategy",
                "octopus",
                "--strategy",
                "recursive",
                "--strategy-option",
                "patience",
                "--strategy-option",
                "ignore-space-change",
            ];
            cmdline.extend(graphed_summands.iter().map(|s| s.node_identity()));

            git_run(repository, &cmdline);


            // "commit": move the SUM head with reflog message:
            force_head_to(
                repository,
                sum.name(),
                // have to sync
                repository
                    .find_branch("temp-sum", BranchType::Local)
                    .unwrap()
                    .get(),
            );
            // git_run("branch", "--force", sum.Name(), tempHead)
            debug!("delete: {:?}", temp_head.name());
            if !git_run(
                repository,
                &["branch", "-D", temp_head.name().unwrap().unwrap()],
            )
                .success()
            {
                panic!("branch -D failed");
            }

        } else {
            // libgit2

            assert!(checkout_new_head_at(repository, None, &first.commit()).is_none());

            // Options:
            let mut merge_opts = MergeOptions::new();
            merge_opts.fail_on_conflict(true)
                .standard_style(true)
                .ignore_whitespace(true)
                .patience(true)
                .minimal(true)
                ;

            let mut checkout_opts = CheckoutBuilder::new();
            checkout_opts.safe();


            // one more conversion:
            // we have Vec<GitHierarchy> ->  Vec<Commit> need..... Vec<AnnotatedCommit>
            //
            let annotated_commits : Vec<AnnotatedCommit<'_>> =
                graphed_summands.iter().skip(1).map(
                    |gh| {
                        let oid = gh.commit().id();
                        repository.find_annotated_commit(oid).unwrap()
                    }).collect();
            // the references vec:
            let annotated_commits_refs = annotated_commits.iter().collect::<Vec<_>>();

            debug!("Calling merge()");
            repository.merge(
                &annotated_commits_refs,
                Some(&mut merge_opts),
                Some(&mut checkout_opts)
            ).expect("Merge should succeed");

            // make if a function:
            // oid = save_index_with( message, signature);
            let mut index = repository.index().unwrap();
            if index.has_conflicts() {
                info!("{}: SORRY conflicts detected", line!());
                exit(1);
            }
            let id = index.write_tree().unwrap();
            let tree = repository.find_tree(id).unwrap();

            let sig = repository.signature().unwrap();


            // Create the commit:
            // another one: Reference -> Commit -> Oid >>> lookup >>> AnnotatedCommit->Oid
            let commits : Vec<Commit<'_>> = graphed_summands.iter()
                .map(|gh| gh.commit())
                .collect();
            // references
            let commits_refs = commits.iter().collect::<Vec<_>>();

            debug!("Calling merge()");

            let new_oid = repository.commit(
                Some("HEAD"),
                &sig, // author(),
                &sig, // committer(),
                &message,
                &tree,
                // this is however already stored in the directory:
                &commits_refs,
                // Error: "failed to create commit: current tip is not the first parent"
            ).unwrap();

            repository.cleanup_state().expect("cleaning up should succeed");

            // this both on the Repo/storer both here in our Data ?
            sum.reference.borrow_mut().set_target(new_oid, "re-merge")
                .expect("should update the symbolic reference -- sum");
        }
    }

    // do we have a hint -- another merge?
    // git merge
    RebaseResult::Done
}

/// Given full git-reference name /refs/remotes/xx/bb return xx and bb
fn extract_remote_name(name: &str) -> (&str, &str) {
    debug!("extract_remote_name: {:?}", name);
    // let norm = Reference::normalize_name(reference.name().unwrap(), ReferenceFormat::NORMAL).unwrap();

    let split_char = '/';

    let (prefix, rest) = name.split_once(split_char).unwrap();
    assert_eq!(prefix, "refs");
    let (prefix, rest) = rest.split_once(split_char).unwrap();
    assert_eq!(prefix, "remotes");

    let (remote, branch) = rest.split_once(split_char).unwrap();
    (remote, branch)
}

fn fetch_upstream_of(repository: &Repository, reference: &Reference<'_>) -> Result<(), Error> {
    // resolve what to fetch.
    if reference.is_remote() {
        let (remote_name, branch) = extract_remote_name(reference.name().unwrap());
        let mut remote = repository.find_remote(remote_name).unwrap();
        debug!("fetching from remote {:?}: {:?}",
               remote.name().unwrap(),
               branch
        );

        // FetchOptions, message
        if remote.fetch(&[branch], None, Some("part of poset-rebasing")).is_err() {
            panic!("** Fetch failed");
        }
    } else if reference.is_branch() {
        // the user has a reason to use local branch.
        // So we don't want to change it (by fetching) without explicit permission.
        // implicit permission -- that it's just following a remove branch.
        let name = Reference::normalize_name(reference.name().unwrap(), ReferenceFormat::NORMAL).unwrap();

        // let b = Branch::wrap(*reference); // cannot move out of `*reference` which is behind a mutable reference
        info!("fetch local {name}");
        // why redo this? see above ^^
        let mut branch = repository
            .find_branch(extract_name(&name), BranchType::Local)
            .unwrap();

        let upstream = branch.upstream().unwrap();
        let upstream_name = upstream.name().unwrap().unwrap();

        // todo: check if still in sync, to not lose local changes.
        if git_same_ref(repository, reference, upstream.get()) {
            debug!("in sync, so let's fetch & update");
        } else {
            panic!("{} not in sync with upstream {}; should not update.", name, upstream_name);
            // or merge/rebase.
        }

        let (rem, br) = divide_str(upstream_name, '/');
        let mut remote = repository.find_remote(rem)?;

        info!("fetch {} {} ....", rem, br);
        if remote.fetch(&[br], None, None).is_ok() {
            let oid = branch
                .upstream()
                .unwrap()
                .get()
                .target()
                .expect("upstream disappeared");
            branch
                .get_mut()
                .set_target(oid, "fetch & fast-forward")
                .expect("fetch/sync failed");
        }
    }
    Ok(())
}

fn rebase_node<'repo>(
    repo: &'repo Repository,
    node: &GitHierarchy<'repo>,
    fetch: bool,
    object_map: &HashMap<String, GitHierarchy<'repo>>,
) {
    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(r) => {
            if fetch {
                fetch_upstream_of(repo, r).expect("fetch failed");
            }
        }
        GitHierarchy::Segment(segment) => {
            let my_span = span!(Level::INFO, "segment", name = segment.name());
            let _enter = my_span.enter();
            rebase_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            remerge_sum(repo, sum, object_map);
        }
    }
}


// ancestor <---is parent-- ........ descendant
fn is_linear_ancestor(repository: &Repository, ancestor: Oid, descendant: Oid) -> bool
{
    if ancestor == descendant { return true;}

    let mut walk = repository.revwalk().unwrap();
    walk.push(descendant).expect("should set upper bound for Walk");
    // segment.reference.borrow().target().unwrap()
    walk.hide(ancestor).expect("should set the lower bound for Walk");

    walk.set_sorting(Sort::TOPOLOGICAL).expect("should set the topo ordering of the Walk");

    if walk.next().is_none() {
        return false;
    }

    for oid in walk {
        if repository.find_commit(oid.unwrap()).unwrap().parent_count() > 1 {
            panic!("a merge found");
        }
    }
    true
}


fn check_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>)
{
    // no merge commits
    if ! is_linear_ancestor(repository,
                            segment.start(),
                            segment.reference.borrow().target().unwrap()) {
        panic!("segment {} in mess", segment.name());
    }

    // no segments inside. lenght limited....

    // git_revisions()
    // walk.push_ref(segment.reference.borrow());
    // walk.hide(segment._start.target().unwrap());
    // walk.hide_ref(ref);

    // push_range
    // descendant of start.

    // start.is_ancestor(reference);
}

fn check_sum<'repo>(
    _repository: &'repo Repository,
    sum: &Sum<'repo>,
    _object_map: &HashMap<String, GitHierarchy<'repo>>,
) {
    let count = sum.reference.borrow().peel_to_commit().unwrap().parent_count();

    // terrible:
    // !i>2 in Rust  means ~i>2 in C
    // https://users.rust-lang.org/t/why-does-rust-use-the-same-symbol-for-bitwise-not-or-inverse-and-logical-negation/117337/2
    if count <= 1 {
        panic!("not a merge: {}, only {} parent commits", sum.name(), count);
    };

    // each of the summands has relationship to a parent commit.
    // divide
    /*
    (mapped, rest_summands, left_overs_parent_commits) = distribute(sum);
    // either it went ahead ....or? what if it's rebased?
    for bad in rest_summands {
        // try to find in over
        find_ancestor()
    }

    */
}


fn check_node<'repo>(
    repo: &'repo Repository,
    node: &GitHierarchy<'repo>,
    object_map: &HashMap<String, GitHierarchy<'repo>>,
) {
    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(_r) => {
            // no
        }
        GitHierarchy::Segment(segment) => {
            check_segment(repo, segment);
        }
        GitHierarchy::Sum(sum) => {
            check_sum(repo, sum, object_map);
        }
    }
}


// whole hierarchy
fn rebase_tree(repository: &Repository,
               root: String,
               fetch: bool,
               ignore: &[String],
               skip: &[String]
) {
    let (
        object_map,    // String -> GitHierarchy
        hash_to_graph, // stable graph:  String -> index ?
        graph,         // index -> String?
        discovery_order,
    ) = find_hierarchy(repository, root);

    // verify we can do it:
    for v in &discovery_order {
        let name = object_map.get(v).unwrap().node_identity();
        println!(
            "{:?} {:?} {:?}",
            v,
            name,
            graph
                .node_weight(*hash_to_graph.get(v).unwrap())
                .unwrap()
        );
        if ignore.iter().any(|x| x == name) {
            eprintln!("found to be ignored {name}");
            continue;
        }
        let vertex = object_map.get(v).unwrap();
        check_node(repository, vertex, &object_map);
    }

    for v in discovery_order {
        let name = object_map.get(&v).unwrap().node_identity();

        if skip.iter().any(|x| x == name) {
            eprintln!("Skipping: {name}");
            continue;
        }

        eprintln!(
            "{:?} {:?} {:?}",
            v,
            object_map.get(&v).unwrap().node_identity(),
            graph
                .node_weight(*hash_to_graph.get(&v).unwrap())
                .unwrap()
        );
        let vertex = object_map.get(&v).unwrap();
        rebase_node(repository, vertex, fetch, &object_map);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short = 'g')]
    directory: Option<PathBuf>,

    #[arg(short='f', long="fetch" )]
    no_fetch: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short, long = "continue")]
    cont: bool,
    root_reference: Option<String>,

    #[arg(short, long = "ignore")]
    ignore: Vec<String>,

    #[arg(short, long = "skip")]
    skip: Vec<String>
}

fn main() {
    let mut cli = Cli::parse();
    // cli can override the Env variable.
    init_tracing(cli.verbose);

    let repository = match Repository::open(cli.directory.unwrap_or(std::env::current_dir().unwrap())) {
        Ok(repository) => repository,
        Err(e) => panic!("failed to open: {}", e),
    };

    // todo:
    // normalize_name(refname: &str, flags: ReferenceFormat) -> Result<String, Error> {

    let root = cli.root_reference
        // if in detached HEAD -- will panic.
        .unwrap_or_else(|| repository.head().unwrap().name().unwrap().to_owned());

    let root = GitHierarchy::Name(root); // not load?
    println!("root is {}", root.node_identity());

    // if file exists -> cli.cont

    if cli.cont {
        rebase_segment_continue(&repository);
    }

    // todo: I must rewrite ignore to full ref names!
    if !cli.skip.is_empty() {
        // rewrite it:
        for e in cli.skip.iter_mut() {
            // rewrite String:
            e.replace_range(..e.len(), repository.resolve_reference_from_short_name(e).unwrap().name().unwrap());
        }
    }

    rebase_tree(&repository, root.node_identity().to_owned(), !cli.no_fetch,
                &cli.ignore,
                &cli.skip);
}
