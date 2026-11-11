mod execution;
mod identity;
mod invocation;
mod reporting;
mod runner;
mod storage;

use clap::Parser;
use invocation::{Args, Invocation};

// By convention, commands like `timeout` use 125 for their own errors. Tests may return it too.
const WRAPPER_ERROR_EXIT_CODE: i32 = 125;

fn main() {
    let invocation = Invocation::from_args(Args::parse());
    match invocation.and_then(runner::run) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("prevtest: {error:#}");
            std::process::exit(WRAPPER_ERROR_EXIT_CODE);
        }
    }
}
