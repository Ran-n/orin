// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:17:18.059360
//+ Revised: 	2026/11/13 15:41:43.739755
// -----------------------------------------------------------------------------

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    if let Err(error) = orin::logging::init() {
        let _ = writeln!(
            io::stderr(),
            "Orin could not configure its diagnostics: {error}"
        );
        return ExitCode::FAILURE;
    }
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            tracing::error!(target: "orin::startup", "{message}");
            let _ = writeln!(io::stderr(), "{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options =
        orin::startup::Options::parse(std::env::args_os().skip(1)).map_err(str::to_owned)?;
    if options.help {
        return write_output(orin::startup::HELP);
    }
    if options.version {
        return write_output(&format!("Orin {}\n", env!("CARGO_PKG_VERSION")));
    }
    orin::desktop::run(options)
}

fn write_output(text: &str) -> Result<(), String> {
    io::stdout()
        .write_all(text.as_bytes())
        .map_err(|_| "Orin could not write its output.".into())
}
