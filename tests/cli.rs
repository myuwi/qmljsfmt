use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::{fs, str};

use tempfile::TempDir;

const UNFORMATTED: &str = "import QtQuick\n\nItem {\n    width: parent.width+1\n}\n";
const FORMATTED: &str = "import QtQuick\n\nItem {\n    width: parent.width + 1\n}\n";
const CLEAN: &str = "import QtQuick\n\nItem {}\n";

fn qmljsfmt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qmljsfmt"))
}

fn run(args: &[&str], input: &str) -> Output {
    let mut command = qmljsfmt();
    command.args(args);

    run_with(command, input)
}

/// Runs the binary inside `directory` so the reported paths stay relative.
fn run_in(directory: &Path, args: &[&str]) -> Output {
    let mut command = qmljsfmt();
    command.current_dir(directory).args(args);

    run_with(command, "")
}

fn run_with(mut command: Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start qmljsfmt");

    let mut stdin = child.stdin.take().expect("stdin was not piped");

    // Invocations rejected for their arguments exit before reading stdin, which
    // closes the pipe under us.
    if let Err(error) = stdin.write_all(input.as_bytes()) {
        assert_eq!(
            error.kind(),
            io::ErrorKind::BrokenPipe,
            "failed to write qmljsfmt input"
        );
    }
    drop(stdin);

    child.wait_with_output().expect("qmljsfmt did not finish")
}

fn stdout(output: &Output) -> &str {
    str::from_utf8(&output.stdout).expect("stdout was not UTF-8")
}

fn stderr(output: &Output) -> &str {
    str::from_utf8(&output.stderr).expect("stderr was not UTF-8")
}

/// A project with an unformatted and a formatted file, plus an ignored one.
fn project() -> TempDir {
    let directory = tempfile::tempdir().expect("failed to create a temporary project");
    let root = directory.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("build")).unwrap();
    fs::write(root.join(".gitignore"), "build/\n").unwrap();
    fs::write(root.join("src/Main.qml"), UNFORMATTED).unwrap();
    fs::write(root.join("src/Clean.qml"), CLEAN).unwrap();
    fs::write(root.join("build/Generated.qml"), UNFORMATTED).unwrap();

    directory
}

fn read(project: &TempDir, path: &str) -> String {
    fs::read_to_string(project.path().join(path)).unwrap()
}

#[test]
fn streams_stdin_to_stdout() {
    let output = run(&["-"], CLEAN);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), CLEAN);
    assert_eq!(stderr(&output), "");
}

#[test]
fn formats_embedded_javascript_on_a_stream() {
    let output = run(&["-"], UNFORMATTED);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), FORMATTED);
    assert_eq!(stderr(&output), "");
}

#[test]
fn checks_a_stream_without_emitting_the_formatted_source() {
    let output = run(&["--check", "-"], UNFORMATTED);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "<stdin>\n");
}

#[test]
fn rejects_invalid_qml_without_emitting_stdout() {
    let output = run(&["-"], "not valid QML\n");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "invalid QML document\n");
}

#[test]
fn reports_missing_oxfmt_without_emitting_stdout() {
    let mut command = qmljsfmt();
    command.arg("-").env("PATH", "/nonexistent");
    // The input needs a JavaScript fragment, or oxfmt is never reached.
    let output = run_with(command, UNFORMATTED);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).starts_with("failed to run oxfmt"));
    assert!(stderr(&output).contains("caused by:"));
}

#[test]
fn rejects_a_stream_combined_with_paths() {
    let output = run(&["-", "src/Main.qml"], "");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("cannot be combined"));
}

#[test]
fn rejects_rewriting_a_stream_in_place() {
    let output = run(&["--write", "-"], CLEAN);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("cannot rewrite a stream in place"));
}

#[test]
fn rejects_unknown_flags_instead_of_treating_them_as_paths() {
    let output = run(&["--nope"], "");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unexpected argument '--nope'"));
}

#[test]
fn rejects_checking_and_writing_at_once() {
    let output = run(&["--check", "--write", "."], "");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("cannot be used with"));
}

#[test]
fn formats_the_current_directory_when_given_no_paths() {
    let project = project();
    let output = run_in(project.path(), &[]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(read(&project, "src/Main.qml"), FORMATTED);
    assert_eq!(read(&project, "src/Clean.qml"), CLEAN);
    assert_eq!(stdout(&output), "");
}

#[test]
fn checks_the_current_directory_when_given_no_paths() {
    let project = project();
    let output = run_in(project.path(), &["--check"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "src/Main.qml\n");
    assert!(stderr(&output).ends_with("1 file would be reformatted, 1 file unchanged\n"));
    // `--check` must not touch the files it reports.
    assert_eq!(read(&project, "src/Main.qml"), UNFORMATTED);
}

#[test]
fn checks_a_formatted_project_cleanly() {
    let project = project();
    fs::write(project.path().join("src/Main.qml"), FORMATTED).unwrap();

    let output = run_in(project.path(), &["--check", "."]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "");
}

#[test]
fn accepts_write_as_the_explicit_default() {
    let project = project();
    let output = run_in(project.path(), &["--write", "src"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(read(&project, "src/Main.qml"), FORMATTED);
}

#[test]
fn skips_ignored_directories_when_walking() {
    let project = project();
    let output = run_in(project.path(), &["--check", "."]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "src/Main.qml\n");
}

#[test]
fn formats_an_ignored_file_when_it_is_named_explicitly() {
    let project = project();
    let output = run_in(project.path(), &["build/Generated.qml"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(read(&project, "build/Generated.qml"), FORMATTED);
}

#[test]
fn formats_a_named_file_whatever_its_extension() {
    let project = project();
    fs::write(project.path().join("src/Odd.txt"), UNFORMATTED).unwrap();

    let output = run_in(project.path(), &["src/Odd.txt"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(read(&project, "src/Odd.txt"), FORMATTED);
}

#[test]
fn counts_a_file_once_when_paths_overlap() {
    let project = project();
    let output = run_in(project.path(), &["--check", "src", "src/Main.qml"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "src/Main.qml\n");
}

#[test]
fn counts_a_file_once_when_reached_through_different_spellings() {
    let project = project();
    let output = run_in(project.path(), &["--check", ".", "src"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "src/Main.qml\n");
}

#[test]
fn reports_paths_that_do_not_exist() {
    let project = project();
    let output = run_in(project.path(), &["--check", "missing.qml"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("failed to read missing.qml"));
}

#[test]
fn reports_when_no_qml_files_are_found() {
    let project = project();
    fs::create_dir_all(project.path().join("empty")).unwrap();

    let output = run_in(project.path(), &["--check", "empty"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("no QML files found"));
}

#[test]
fn survives_a_reader_that_closes_the_pipe() {
    let project = project();
    let mut command = qmljsfmt();
    let mut child = command
        .current_dir(project.path())
        .arg("--check")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start qmljsfmt");

    drop(child.stdout.take());

    let output = child.wait_with_output().expect("qmljsfmt did not finish");
    assert_eq!(output.status.code(), Some(1));
}
