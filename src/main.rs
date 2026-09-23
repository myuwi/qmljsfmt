use std::collections::HashSet;
use std::error::Error;
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, thread};

use clap::Parser;
use ignore::WalkBuilder;

type CliResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const QML_EXTENSION: &str = "qml";

/// Formats JavaScript embedded in QML documents.
#[derive(Parser)]
#[command(name = "qmljsfmt", version, about)]
struct Cli {
    /// Check if files are formatted
    #[arg(short, long, conflicts_with = "write")]
    check: bool,

    /// Rewrite files in place (default)
    #[arg(short, long)]
    write: bool,

    /// Files and directories to format, or `-` for stdin
    #[arg(value_name = "PATH", default_value = ".")]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli) {
        Ok(unformatted) => ExitCode::from(u8::from(unformatted)),
        Err(error) => {
            report_error(None, &*error);
            ExitCode::from(2)
        }
    }
}

fn report_error(path: Option<&Path>, error: &dyn Error) {
    if let Some(path) = path {
        eprint!("{}: ", path.display());
    }
    eprintln!("{error}");

    let mut cause = error.source();
    while let Some(error) = cause {
        eprintln!("  caused by: {error}");
        cause = error.source();
    }
}

/// Reports whether `--check` found unformatted files.
fn run(cli: &Cli) -> CliResult<bool> {
    if cli.paths.iter().any(|path| path.as_os_str() == "-") {
        if cli.paths.len() > 1 {
            return Err("`-` cannot be combined with other paths".into());
        }

        if cli.write {
            return Err("cannot rewrite a stream in place".into());
        }

        return run_stream(cli.check);
    }

    let files = walk(&cli.paths)?;
    if files.is_empty() {
        return Err("no QML files found in the given paths".into());
    }

    run_files(&files, cli.check)
}

fn run_stream(check: bool) -> CliResult<bool> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let output = qmljsfmt::format(&input)?;

    if !check {
        io::stdout().write_all(output.as_bytes())?;
        return Ok(false);
    }

    if output == input {
        return Ok(false);
    }

    println!("<stdin>");

    Ok(true)
}

/// Drops the `./` the walker prefixes onto everything under a `.` root.
fn strip_dot(path: PathBuf) -> PathBuf {
    match path.strip_prefix("./") {
        Ok(stripped) => stripped.to_path_buf(),
        Err(_) => path,
    }
}

fn walk(paths: &[PathBuf]) -> CliResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();

    for path in paths {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        // A file named explicitly is kept whatever its extension, and whether or not it is ignored.
        if !metadata.is_dir() {
            let path = strip_dot(path.clone());
            if seen.insert(path.clone()) {
                files.push(path);
            }
            continue;
        }

        let entries = WalkBuilder::new(path)
            .follow_links(false)
            .require_git(false)
            .sort_by_file_path(Path::cmp)
            .build();

        for entry in entries {
            let entry = entry?;

            if !entry.file_type().is_some_and(|entry| entry.is_file())
                || entry.path().extension() != Some(OsStr::new(QML_EXTENSION))
            {
                continue;
            }

            let path = strip_dot(entry.into_path());
            if seen.insert(path.clone()) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn run_files(files: &[PathBuf], check: bool) -> CliResult<bool> {
    let (mut changed, mut failed) = (0, 0);
    let mut out = io::stdout().lock();
    let mut listing = check;

    for (path, result) in files.iter().zip(format_files(files, check)) {
        match result {
            Ok(false) => (),
            Ok(true) => {
                changed += 1;

                if listing {
                    // A reader that closed the pipe early (e.g. `| head`) ends the listing.
                    listing = writeln!(out, "{}", path.display()).is_ok();
                }
            }
            Err(error) => {
                failed += 1;
                report_error(Some(path), &*error);
            }
        }
    }

    let verb = if check { "would be " } else { "" };
    let unchanged = files.len() - changed - failed;
    // The summary is prose, so it stays off the list on stdout.
    eprintln!(
        "{changed} {} {verb}reformatted, {unchanged} {} unchanged",
        format_file_count(changed),
        format_file_count(unchanged)
    );

    if failed > 0 {
        return Err(format!(
            "{failed} {} could not be formatted",
            format_file_count(failed)
        )
        .into());
    }

    Ok(check && changed > 0)
}

fn format_file_count(count: usize) -> &'static str {
    if count == 1 { "file" } else { "files" }
}

fn format_files(files: &[PathBuf], check: bool) -> Vec<CliResult<bool>> {
    // Each file costs an oxfmt subprocess, so the work is worth spreading.
    let threads = thread::available_parallelism().map_or(1, |threads| threads.get());
    let chunk = files.len().div_ceil(threads).max(1);

    thread::scope(|scope| {
        // Chunks are handed out in order and joined in order, so the results line
        // up with `files` however the threads interleave.
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|path| format_file(path, check))
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("formatting thread panicked"))
            .collect()
    })
}

fn format_file(path: &Path, check: bool) -> CliResult<bool> {
    let input = fs::read_to_string(path)?;
    let output = qmljsfmt::format(&input)?;

    if output == input {
        return Ok(false);
    }

    if !check {
        fs::write(path, &output)?;
    }

    Ok(true)
}
