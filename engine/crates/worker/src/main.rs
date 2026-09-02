//! flux-worker: the Rust data-plane worker binary.
//!
//! Hosts the SQL frontend, vectorized execution engine, and shuffle services.
//! See docs/04-详细设计说明书.md for the full design.

use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("flux-worker: fatal: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    println!("flux-worker {} starting", env!("CARGO_PKG_VERSION"));
    Ok(())
}
