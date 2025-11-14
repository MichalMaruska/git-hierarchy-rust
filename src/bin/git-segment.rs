use std::path::PathBuf;
use clap::{Parser,Subcommand,CommandFactory,FromArgMatches};
use git2::Repository;

#[allow(unused_imports)]
use git_hierarchy::git_hierarchy::{Segment,segments};


/// Operate on segments or 1 segment
#[derive(Parser)] // Debug
// about ... Description from Cargo.toml
#[command(version, long_about = None)]
#[command(subcommand_negates_reqs = true)]
#[command(args_conflicts_with_subcommands = true)] // positional arguments
// ^^ this means that Factory produces command, and then ^^ those are called on it?
struct Cli {

    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[command(flatten)]
    git_repository: ClapGitRepo,

    #[command(subcommand)]
    #[command(name="subcommand")]
    // expand shows:
    // .subcommand_required(false)
    // .arg_required_else_help(false);
    command: Option<Commands>,

    #[command(flatten)]
    #[command(name="define")]
    define_args: Option<DefineArgs>,
}


#[derive(clap::Args)]
#[command(name="git", about = None, long_about = None)]
struct ClapGitRepo {
    #[arg(long, short='g')]
    #[arg(global=true)]
    // why option? b/c otherwise .required(!has_default)
    directory: Option<PathBuf>,
}


#[derive(Subcommand)]
enum Commands {
    List(ListArgs),
    Restart(RestartArgs),
    Update(RebaseArgs),
    Delete(DeleteCmd),
    Define(DefineArgs),
}


/// listing all segments
#[derive(clap::Args)]
// why do I have this, and not #[arg()]?
#[command(version, about, long_about = None,long_flag("list"),short_flag('l'))]
#[command(name="list")]
struct ListArgs {
    #[arg(long, short,group = "format")]
    short: bool,

    // inverse?
    #[arg(long, short,group = "format")]
    // action = clap::SetTrue
    full: bool,

    #[arg(long, short='p')]
    diff: bool,
}



#[derive(clap::Args)]
#[command(version, about, long_about = None,long_flag("restart"),short_flag('r'))]
struct RestartArgs {
    segment_name: String,
    commit: String,
}

#[derive(clap::Args)]
#[command(version, about, long_about = None,long_flag("update"),short_flag('u'))]
struct RebaseArgs {
    #[arg(long="base", short='b')]
    rebase: bool,
    segment_name: String,
    new_base: String,
}


#[derive(clap::Args)]
#[command(version, about, long_about = None,long_flag("delete"),short_flag('d'))]
struct DeleteCmd {
    segment_name: String,
}

// I want this default.... can I flatten it in?
#[derive(clap::Args)]
#[allow(unused_variables)]
// so for Args I can have command? Does it call it during the augment_args(command) call?
#[command(name="define", version, about, long_about = None,long_flag("define"),short_flag('D'))]
struct DefineArgs {
    #[arg(long, short)]
    checkout: bool,

    segment_name: String,
    base: String,
    start: Option<String>, // default to @base
    head: Option<String>,
}

fn define<'repo> (repository: &'repo Repository, args: &DefineArgs) -> Result<Segment<'repo>, git2::Error>
{
    let base = repository.resolve_reference_from_short_name(&args.base)?;
    // reference? cannot clone base
    let start = args.start.as_ref().map_or(repository.resolve_reference_from_short_name(base.name().unwrap()).unwrap(),
                                           |name|
                                           repository.resolve_reference_from_short_name(&name).unwrap());
    let head = repository.resolve_reference_from_short_name(
        args.head.as_ref().map_or("HEAD",
                                  |x| &x)).unwrap();
    let res = Segment::create(&repository, &args.segment_name, &base, &start, &head);

    let hash = start.target().unwrap();
    // todo: show it
    println!("create {} in {:?}", args.segment_name, repository.path());
    println!("base = {}, start {} = {}", base.name().unwrap(), start.name().unwrap(), hash.to_string());
    res
}

fn delete(repository: &Repository, args: &DeleteCmd) {
    println!("would delete {} in {:?}", args.segment_name, repository.path());
}

fn describe(repository: &Repository, segment: &str) {
    println!("Segment {} in {:?}", segment, repository.path());
}

fn list_segments(repository: &Repository) {
    let ref_iterator = segments(&repository);

    for r in ref_iterator {
        println!("{}", r);
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {

    let clip =
        if true {
            let mut cli = Cli::command();
            //
            cli = cli.subcommand_negates_reqs(true);

            // get_matches, -> ArgMatches
            // Parser trait ... FromArgMatches ...

            if false {
                for i in cli.get_opts() {
                    println!("option {i:?}");
                    // -> impl Iterator<Item = &Arg>
                }
            }

            let mut matches = cli.get_matches();
            // clip = Cli::parse();

            // this fails... MissingRequiredArgument b/c define_args
            Cli::from_arg_matches_mut(&mut matches).expect("assignment failed")
        } else {
            Cli::parse()
        };

    tracing_subscriber::fmt()
        .with_max_level(clip.verbosity)
        .init();

    /*
    let args: Vec<String> = env::args().collect();
    */

    let repository = match clip.git_repository.directory {
        None => Repository::open_from_env().expect("failed to find Git repository"),
        Some(dir) => Repository::open(dir).expect("failed to find Git repository"),
    };

    // this is an associated function, not a method
    if let Some(command) = clip.command {
        match command {
            Commands::List(_args) => {
                list_segments(&repository);
            }
            Commands::Restart(args) => {
                println!("would restart from {}", args.commit);
            },
            Commands::Update(args) => {
                println!("would rebase from {} {}", args.new_base,
                         if args.rebase {"immediately"} else {""});
            },
            Commands::Delete(args) => {
                delete(&repository, &args);
            },
            Commands::Define(args) => {
                define(&repository, &args).expect("failed to define new segment");
            },
        }
    } else if let Some(args) = clip.define_args {
        define(&repository, &args);
    }
    // else nothing. Or list?
    // return Err(error.into());
    Ok(())
}
