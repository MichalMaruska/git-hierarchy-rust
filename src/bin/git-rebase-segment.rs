// todo: global?
#![deny(elided_lifetimes_in_paths)]

use git2::{Repository};
use std::path::PathBuf;
use ::git_hierarchy::git_hierarchy::{GitHierarchy};
use ::git_hierarchy::rebase::{check_segment, rebase_segment};
use ::git_hierarchy::utils::{init_tracing};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short = 'g')]
    directory: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    segment_name: String,
}

// should we check the segment first?
fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let repository = match Repository::open(cli.directory.unwrap_or(std::env::current_dir().unwrap())) {
        Ok(repository) => repository,
        Err(e) => panic!("failed to open: {}", e),
    };

    let gh = git_hierarchy::git_hierarchy::load(&repository, &cli.segment_name).unwrap();
    if let GitHierarchy::Segment(segment) = gh {
        check_segment(&repository, &segment);
        rebase_segment(&repository, &segment);
    }
}
