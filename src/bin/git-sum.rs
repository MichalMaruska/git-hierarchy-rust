use std::path::PathBuf;
use clap::{Parser,Subcommand};
use git2::{Repository,Reference};

#[allow(unused_imports)]
use git_hierarchy::git_hierarchy::{GitHierarchy,Sum,load,sums};


/*
${0##* }
  list all sums

${0##*} [-c] {name}
  show the definition -- list of summands

  -c ... prune non-existings summands
  # does this make it contiguous?

${0##*} [-s start] [-r] branch new-merge-branch -drop-merge-branch ...
  modify the definition -- stepwise?
  -r reset: empty the definition first.
  -s start_point
  -n do NOT merge (yet)

  -name ... remove this ref as summand
  name | +name add.

  -m  number the summands

${0##*} [-d] branch
  drop the definition.
*/






/// Manage Sum information -- merge definitions
#[derive(Parser)]
#[command(version, long_about = None)]
#[command(subcommand_negates_reqs = true)]
#[command(args_conflicts_with_subcommands = true)] // positional arguments
// ^^ this means that Factory produces command, and then ^^ those are called on it?
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    // move elsewhere
    #[command(flatten)]
    git_repository: ClapGitRepo,

    #[command(subcommand)]
    #[command(name="subcommand")]
    command: Option<Commands>,

    define_or_show_args: Option<Vec<String>>,
}

#[derive(clap::Args)]
#[command(name="git", about = None, long_about = None)]
struct ClapGitRepo {
    #[arg(long, short='g')]
    #[arg(global=true)]
    directory: Option<PathBuf>,
}


#[derive(Subcommand)]
enum Commands {
    #[command(name="list", version, about, long_about = None,
              long_flag("list"),short_flag('l'))]
    List(ListArgs),

    #[command(name="delete", version, about, long_about = None,
              long_flag("delete"),short_flag('d'))]
    Delete(DeleteCmd),

    #[command(name="define", version, about, long_about = None,long_flag("define"),short_flag('D'))]
    Define(DefineArgs),
}


#[derive(clap::Args)]
// why do I have this, and not #[arg()]?
struct DefineArgs
{
    #[arg(long, short ='h')]
    head: Option<String>,

    name: String,
    components: Vec<String>,
}

/// listing all segments
#[derive(clap::Args)]
struct ListArgs {
    #[arg(long, short,group = "format")]
    short: bool,

    // inverse?
    #[arg(long, short,group = "format")]
    // action = clap::SetTrue
    full: bool,

    #[arg(long, short='p')]
    diff: bool,

    name: Option<String>,
}


#[derive(clap::Args)]
#[command(version, about, long_about = None,long_flag("delete"),short_flag('d'))]
struct DeleteCmd {
    sum_name: String,
}

fn main()
{
    let clip = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(clip.verbosity)
        .init();

    let repository = match clip.git_repository.directory {
        None => Repository::open_from_env().expect("failed to find Git repository"),
        Some(dir) => Repository::open(dir).expect("failed to find Git repository"),
    };


    if let Some(command) = clip.command {
        match command {
            Commands::List(_args) => {
                println!("list");
                list_sums(&repository);
            }
            Commands::Define(_args) => {
                unimplemented!();
            }
            Commands::Delete(args) => {
                println!("would delete {}", args.sum_name);
                unimplemented!()
            }
        }
    } else if let Some(args) = clip.define_or_show_args {
        if args.len() == 1 {
            let gh = git_hierarchy::git_hierarchy::load(&repository, &args[0]).unwrap();
            if let GitHierarchy::Sum(sum) = gh {
                describe_sum(&repository, &sum);
            }
        } else {
/*
            let d = DefineArgs {
                head: None,
                name: args[0], // take
                components: args[1..],
            }
*/
            // println!("would create {} from {}", args.);
            let summands : Vec<Reference>
                               = args.iter().skip(1).map(|x| {
                    repository.resolve_reference_from_short_name(&x).unwrap()
            }).collect();
            Sum::create(
                &repository,
                &args[0],
                summands.iter(),
                    // map(|x| &x),
                None // d.head,
            );
        }
    } else {
        list_sums(&repository);
    }
}

fn list_sums(repository: &Repository) {
    let ref_iterator = sums(&repository);

    for r in ref_iterator {
        println!("{}", r);
    }
}

fn describe_sum<'repo>(repository: &'repo Repository, sum: &git_hierarchy::git_hierarchy::Sum<'repo>) {
    println!("sum {}", sum.name());
    let summands = sum.summands(repository);
    for s in &summands {
        println!("\t {}", s.name().unwrap());
    }
    // report if clean or dirty.


    // prune non-existings summands ??? why?
    // fn show_prune_definition(){unimplemented!()}
}


fn git_sum_branches() {unimplemented!()}

fn add_to_sum(){unimplemented!()}

fn remove_from_sum() {unimplemented!()}
