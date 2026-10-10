//! ConfigLens binary entry point.
//!
//! Installations: the panic hook prints a constant string, so no input content can reach the
//! terminal through a panic (spec §7).

#![forbid(unsafe_code)]

use clap::Parser;

use configlens::cli::{run, Cli};

fn main() {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("configlens: internal error (input content is not shown)");
    }));
    let cli = Cli::parse();
    std::process::exit(run(cli));
}
