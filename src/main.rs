//! ConfigLens binary entry point.
//!
//! Installations: the panic hook prints a constant string, so no input content can reach the
//! terminal through a panic (spec §7).

#![forbid(unsafe_code)]

use clap::{error::ErrorKind, Parser};
use std::io::Write;

use configlens::cli::{run, Cli};

fn main() {
    std::panic::set_hook(Box::new(|_| {
        let _ = writeln!(
            std::io::stderr().lock(),
            "configlens: internal error (input content is not shown)"
        );
    }));
    let code = match Cli::try_parse() {
        Ok(cli) => run(cli),
        Err(error) => match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                // Help/version contain only the static command contract. Clap's
                // fallible printer also permits a consumer to close stdout.
                match error.print() {
                    Ok(()) => 0,
                    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => 0,
                    Err(_) => 4,
                }
            }
            _ => {
                // Clap diagnostics can echo arbitrary argv, including secrets;
                // sanitizing terminal controls alone cannot make them public.
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "configlens: usage: invalid command arguments; see --help"
                );
                2
            }
        },
    };
    std::process::exit(code);
}
