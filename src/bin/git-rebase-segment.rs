#![deny(elided_lifetimes_in_paths)]

use std::path::PathBuf;
use clap::Parser;

use git_hierarchy::git_hierarchy::{GitHierarchy};
use git_hierarchy::rebase::{check_segment, rebase_segment};
use git_hierarchy::utils::{init_tracing};
use git_hierarchy::base::open_repository;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(long, short = 'g')]
    directory: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    // todo: continue -> use git-rebase-poset -c
    // should this be an invocation of git-rebase-poset?
    segment_name: String,
}

// should we check the segment first?
fn main() -> Result<(), Box<dyn std::error::Error>>{
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let repository = match open_repository(cli.directory.as_ref())
    {
        Ok(repository) => repository,
        Err(e) => panic!("failed to open: {}", e),
    };

    // continue...
    let gh = git_hierarchy::git_hierarchy::load(&repository, &cli.segment_name).unwrap();
    if let GitHierarchy::Segment(segment) = gh {
        check_segment(&repository, &segment)?;
        rebase_segment(&repository, &segment).unwrap();
    }
    Ok(())
}
