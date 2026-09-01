use std::error::Error;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use qmljsfmt::Result;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");

            let mut cause = error.source();
            while let Some(error) = cause {
                eprintln!("  caused by: {error}");
                cause = error.source();
            }

            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let output = qmljsfmt::format(&input)?;
    io::stdout().write_all(output.as_bytes())?;

    Ok(())
}
