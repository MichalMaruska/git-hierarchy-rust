use git2::Repository;
pub use std::process::{Command, ExitStatus};

#[allow(unused)]
use tracing::{debug, info, warn, error};

/// Invoke git with the given CLI arguments. In the directory of the @repository.
pub fn git_run(repository: &Repository, cmd_line: &[&str]) -> ExitStatus {
    let mut command = Command::new("git");
    command.args(cmd_line);
    command.current_dir(repository.workdir().unwrap());
    debug!("must cd into {}", repository.workdir().unwrap().display());
    warn!("git-run: {}", cmd_line.join(" "));

    let child = command.spawn().expect("git command failed to start");
    let output = child.wait_with_output().expect("Failed to wait on git");

    dbg!(output.status);
    output.status
}
