use git2::Repository;
pub use std::process::{Command, ExitStatus};

#[allow(unused)]
use tracing::{debug, info, warn, error};


pub enum Error {
    NoWorkDir,
    ProcessError(std::io::Error),
}

/// Invoke git with the given CLI arguments. In the directory of the @repository.
pub fn git_run(repository: &Repository, cmd_line: &[&str]) -> Result<ExitStatus, Error> {
    let mut command = Command::new("git");
    command.args(cmd_line);

    command.current_dir(repository.workdir().ok_or(Error::NoWorkDir)?);
    debug!("must cd into {}", repository.workdir().unwrap().display());
    warn!("git-run: {}", cmd_line.join(" "));

    let child = command.spawn().expect("git command failed to start");
    let output = child.wait_with_output().expect("Failed to wait on git");

    dbg!(output.status);
    Ok(output.status)
}
