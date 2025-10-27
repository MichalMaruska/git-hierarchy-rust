use std::env;
use std::path::PathBuf;
use clap::Parser;
use git2::Repository;

use git_hierarchy::git_hierarchy::Segment;


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[arg(long, short='g')]
    directory: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .init();

    let args: Vec<String> = env::args().collect();
    let name = &args[1];
    let reference_name = &args[2];

    let repository = match cli.directory {
        None => Repository::open_from_env().expect("failed to find Git repository"),
        Some(dir) => Repository::open(dir).expect("failed to find Git repository"),
    };

    let reference = repository.resolve_reference_from_short_name(reference_name).unwrap();

    Segment::create(&repository, name, &reference, &reference, &reference).unwrap();

    Ok(())
}
