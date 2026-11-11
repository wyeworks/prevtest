//! Test execution, output capture, and replay.

use anyhow::{Context, Result};
use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Write},
    os::unix::process::ExitStatusExt,
    process::{Command, ExitStatus, Stdio},
    thread,
};

pub struct CapturedOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub struct Execution {
    pub status: ExitStatus,
    pub output: CapturedOutput,
}

impl Execution {
    pub fn exit_code(&self) -> i32 {
        self.status
            .code()
            .unwrap_or_else(|| 128 + self.status.signal().unwrap_or(0))
    }
}

pub fn run(executable: &OsStr, args: &[OsString]) -> Result<Execution> {
    let mut child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {}", executable.to_string_lossy()))?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    thread::scope(|scope| {
        let out = scope.spawn(|| capture(stdout, io::stdout().lock()));
        let err = scope.spawn(|| capture(stderr, io::stderr().lock()));
        let status = child.wait();
        let stdout = out.join().expect("stdout reader panicked");
        let stderr = err.join().expect("stderr reader panicked");
        Ok(Execution {
            status: status.context("waiting for test")?,
            output: CapturedOutput {
                stdout: stdout.context("capturing stdout")?,
                stderr: stderr.context("capturing stderr")?,
            },
        })
    })
}

fn capture(mut reader: impl Read, mut writer: impl Write) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 16 * 1024];
    let mut write_error = None;
    loop {
        let count = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if write_error.is_none() {
            write_error = writer
                .write_all(&buffer[..count])
                .and_then(|()| writer.flush())
                .err();
        }
        // Keep draining after a write error so the child cannot block on a full pipe.
    }
    match write_error {
        Some(error) => Err(error),
        None => Ok(bytes),
    }
}

pub fn replay(output: &CapturedOutput) -> Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(&output.stdout)?;
    stdout.flush()?;
    let mut stderr = io::stderr().lock();
    stderr.write_all(&output.stderr)?;
    stderr.flush()?;
    Ok(())
}
