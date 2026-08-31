use std::io::{self, Read, Write};
use std::process::ExitCode;

use qmljsfmt::Result;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
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
