//! Parses command-line arguments and environment variables into a test invocation.

use anyhow::{Context, Result, ensure};
use clap::{Parser, builder::BoolishValueParser};
use std::{env, ffi::OsString, path::PathBuf};

const DEFAULT_CACHE_DIR: &str = ".prevtest/cache";

#[derive(Parser)]
#[command(version, about = "Reuse previous test results with nextest.")]
pub struct Args {
    /// Execute again and replace any cached result for this key.
    // Match nextest's boolean environment variable parsing.
    #[arg(long, env = "PREVTEST_FORCE", value_parser = BoolishValueParser::new())]
    force: bool,

    /// Test executable followed by its arguments.
    #[arg(required = true, num_args = 1.., trailing_var_arg = true)]
    command: Vec<OsString>,
}

pub struct Invocation {
    pub executable: OsString,
    pub args: Vec<OsString>,
    pub binary_id: String,
    pub test_name: String,
    pub workspace: PathBuf,
    pub working_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub force: bool,
    pub report_path: Option<PathBuf>,
}

impl Invocation {
    pub fn from_args(args: Args) -> Result<Self> {
        ensure!(
            env::var("NEXTEST_EXECUTION_MODE").as_deref() == Ok("process-per-test"),
            "prevtest must be configured as a nextest run wrapper"
        );
        let workspace = PathBuf::from(
            env::var_os("NEXTEST_WORKSPACE_ROOT").context("NEXTEST_WORKSPACE_ROOT is missing")?,
        );
        ensure!(workspace.is_absolute(), "workspace root must be absolute");
        let workspace = workspace
            .canonicalize()
            .context("resolving workspace root")?;
        let cache_dir = workspace
            .join(env::var_os("PREVTEST_CACHE_DIR").unwrap_or_else(|| DEFAULT_CACHE_DIR.into()));
        let mut command = args.command.into_iter();
        Ok(Self {
            executable: command.next().context("test executable is missing")?,
            args: command.collect(),
            binary_id: env::var("NEXTEST_BINARY_ID").context("NEXTEST_BINARY_ID is missing")?,
            test_name: env::var("NEXTEST_TEST_NAME").context("NEXTEST_TEST_NAME is missing")?,
            workspace,
            working_dir: env::current_dir()?,
            cache_dir,
            force: args.force
                || env::var("NEXTEST_ATTEMPT").is_ok_and(|value| value != "1")
                || env::var("NEXTEST_STRESS_CURRENT").is_ok_and(|value| value != "none"),
            report_path: env::var_os("NEXTEST_RUN_WRAPPER_REPORT").map(PathBuf::from),
        })
    }
}
