pub mod build;
pub mod device;
pub mod identity;
pub mod install;
pub mod intent;
pub mod sign;
pub mod toolchain;

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;
use std::process::{Command, Output};

#[derive(Debug)]
pub struct CommandError {
    pub program: String,
    pub output: Output,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        write!(
            f,
            "{} exited with {}: {}",
            self.program,
            self.output.status,
            stderr.trim()
        )
    }
}

impl std::error::Error for CommandError {}

pub(crate) fn run_checked<I, S>(
    program: &str,
    arguments: I,
    cwd: Option<&Path>,
) -> Result<Output, CommandError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let arguments: Vec<OsString> = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_os_string())
        .collect();
    let mut command = Command::new(program);
    command.args(&arguments);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command.output().map_err(|error| CommandError {
        program: program.into(),
        output: synthetic_failure(error),
    })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(CommandError {
            program: format!(
                "{} {}",
                program,
                arguments
                    .iter()
                    .map(|argument| argument.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            output,
        })
    }
}

#[cfg(unix)]
fn synthetic_failure(error: std::io::Error) -> Output {
    use std::os::unix::process::ExitStatusExt;
    Output {
        status: std::process::ExitStatus::from_raw(1 << 8),
        stdout: Vec::new(),
        stderr: error.to_string().into_bytes(),
    }
}
