// walk the hierarchy
// - assemble list of segments/sums.
// todo:
// - clone
// - replaceInHierarchy ...the base from->to, mapping

use clap::Parser;
use git2::{Repository,Reference};

use std::collections::HashMap;
use std::path::PathBuf;

use ::git_hierarchy::utils::init_tracing;
/*
 note: ambiguous because of a conflict between a name from a glob
       import and an outer scope during import or macro resolution
   = note: `git_hierarchy` could refer to a crate passed with `--extern`
   = help: use `::git_hierarchy` to refer to this crate unambiguously
*/

use ::git_hierarchy::graph::discover::NodeExpander;
use ::git_hierarchy::graph::discover_pet::find_hierarchy;

#[allow(unused)]
use ::git_hierarchy::git_hierarchy::{GitHierarchy, Segment, Sum, load};

#[allow(unused)]
use tracing::{debug, info};


fn list_segment<'repo>(repository: &'repo Repository, segment: &Segment<'repo>) {
    let walk = segment.iter(repository).unwrap();
    for c in walk {
        let oid = c.unwrap();
        let commit = repository.find_commit(oid).unwrap();
        let message = commit.message().unwrap();
        println!("{:?}: {}", oid, message.lines().next().unwrap());
    }
    println!();
}

fn process_node<'repo>(
    repository: &'repo Repository,
    node: &GitHierarchy<'repo>,
    _object_map: &HashMap<String, GitHierarchy<'repo>>, // _remapped : HashMap<String, String>,
) {
    debug!(
        "{:?}",
        // object_map.get(&v).unwrap()
        node.node_identity(),
        // object_map
        // graph.node_weight(hash_to_graph.get(node).unwrap().clone()).unwrap()
    );

    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(r) => {
            println!("a ref {}", r.name().unwrap());
        }
        GitHierarchy::Segment(segment) => {
            let base = segment.base(repository);
            // let start = &segment._start;
            // start == base.peel_to_commit().unwrap())

            // target
            let state;

            if segment.uptodate(repository) {
                state = "up-to-date";
            } else {
                state = "need-rebase";
            }
            println!(
                "segment {}: {:?} on {:?}",
                segment.name(),
                base.name().unwrap(),
                state // base.peel_to_commit().unwrap().id(),
            );

            list_segment(repository, segment);
        }
        GitHierarchy::Sum(sum) => {
            let summands = sum.summands(repository);

            println!("a sum of: ");
            for s in &summands {
                println!("  {}", s.name().unwrap());
            }
        }
    }
}

fn rename_nodes<'repo>(
    repository: &'repo Repository,
    node: &GitHierarchy<'repo>,
    _object_map: &HashMap<String, GitHierarchy<'repo>>,
    remapped: &mut HashMap<String, String>,
) {
    debug!(
        "{:?}",
        // object_map.get(&v).unwrap()
        node.node_identity(),
        // object_map
        // graph.node_weight(hash_to_graph.get(node).unwrap().clone()).unwrap()
    );

    match node {
        GitHierarchy::Name(_n) => {
            panic!();
        }
        GitHierarchy::Reference(r) => {
            println!("a ref {}", r.name().unwrap());
        }
        GitHierarchy::Segment(segment) => {
            // if segment itself in replace ... ignore it.
            if remapped.get(segment.reference.borrow().name().unwrap()).is_some() {
                info!("this segment is itself to be replaced, so ignoring");
                return;
            }

            let base = segment.base(repository);
            let base_name = base.name().unwrap();

            if let Some(replacement) = remapped.get(base_name) {
                debug!("exchange base {}", base_name);
                segment.base.borrow_mut().symbolic_set_target(replacement, "replacement")
                    .expect("should be possible to change Base symbolic reference");
            }
        }
        GitHierarchy::Sum(sum) => {
            let summands = sum.summands(repository);

            println!("a sum of: ");
            for s in &summands {
                let name = s.name().unwrap();
                println!("{}", name);

                if remapped.get(name).is_some() {
                    println!("Would change the summand {}", name);
                }
            }
        }
    }
}


fn register_for_replacement<'repo>(
    remapped: &mut HashMap<String, String>,
    from: &Reference<'repo>,
    target: &Reference<'repo>,
)
{
    let name = from.name().unwrap().to_owned();
    let target = target.name().unwrap().to_owned();
    info!("will replace {} with {}", &name, &target);
    remapped.insert(name, target).and_then(|_ : String| -> Option<String> {panic!("double")});
    debug!("hash: {remapped:?}");
}


fn walk_down<F>(repository: &Repository, root: &str, mut process: F)
where
    F: for<'repo, 'a> FnMut(
        &'repo git2::Repository,
        &GitHierarchy<'repo>,
        &'a HashMap<String, GitHierarchy<'repo>>,
    ) -> (), // F : FnMut(&Repository, GitHierarchy,
             //          &HashMap<String, GitHierarchy>) -> ()
{
    let (
        object_map,     // String -> GitHierarchy
        _hash_to_graph, // stable graph:  String -> index ?
        _graph,         // index -> String?
        discovery_order,
    ) = find_hierarchy(repository, root.to_owned());

    // convert the gh objects?
    for v in discovery_order {
        let vertex = object_map.get(&v).unwrap();
        process(repository, vertex, &object_map);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short='g')]
    directory: Option<PathBuf>,

    root_reference: Option<String>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[arg(long, short = 'r', num_args(2))]
    rename: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    init_tracing(cli.verbose);

    if !cli.rename.is_empty() {
        if cli.rename.len() != 2 {
            panic!("--rename takes 2 parameters");
        }
        // also, in this case I don't start *implicitly* by HEAD.
        if cli.root_reference.is_none() {
            panic!("when --rename is used, the top must be stated")
        }
    }

    let repository = match Repository::open(cli.directory.unwrap_or(std::env::current_dir().unwrap())) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    let root = cli
        .root_reference
        .unwrap_or_else(|| {
            let head = repository.head().unwrap().name().unwrap().to_owned();
            // let head = repo.head().unwrap().name().unwrap();
            //                       ^^^
            // creates a temporary value which is freed while still in use

            // what? that is no more temporary?
            // let head = repo.head().unwrap();
            // let head = head.name().unwrap().to_owned();
            info!("Start from the HEAD = {}", &head);
            head.to_owned()
        });

    if !cli.rename.is_empty() {
        info!("Renaming");
        // resolve them...
        let mut remapped = HashMap::new();

        let from = repository.resolve_reference_from_short_name(&cli.rename[0]).unwrap();
        let target = repository.resolve_reference_from_short_name(&cli.rename[1]).unwrap();
        register_for_replacement(&mut remapped, &from, &target);
        // move object_map ?
        walk_down(&repository, &root, |repository, node, object_map| {
            rename_nodes(repository, node, object_map, &mut remapped)
        });
    } else {
        walk_down(&repository, &root, process_node);
    }
}
