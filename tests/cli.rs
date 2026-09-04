use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(input: &str) -> Output {
    run_with(Command::new(env!("CARGO_BIN_EXE_qmljsfmt")), input)
}

fn run_with(mut command: Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start qmljsfmt");

    child
        .stdin
        .take()
        .expect("stdin was not piped")
        .write_all(input.as_bytes())
        .expect("failed to write qmljsfmt input");

    child.wait_with_output().expect("qmljsfmt did not finish")
}

#[test]
fn reports_missing_oxfmt_without_emitting_stdout() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qmljsfmt"));
    command.env("PATH", "/nonexistent");
    let output = run_with(
        command,
        "import QtQuick\n\nItem {\n    width: parent.width\n}\n",
    );

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stderr.starts_with("failed to run oxfmt"));
    assert!(stderr.contains("caused by:"));
}

#[test]
fn passes_stdin_through_to_stdout() {
    let input = "import QtQuick\n\nItem {}\n";
    let output = run(input);

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), input);
    assert!(output.stderr.is_empty());
}

#[test]
fn formats_embedded_javascript() {
    let output = run("import QtQuick\n\nItem {\n    width: parent.width+1\n}\n");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "import QtQuick\n\nItem {\n    width: parent.width + 1\n}\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn rejects_invalid_qml_without_emitting_stdout() {
    let output = run("not valid QML\n");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "invalid QML document\n"
    );
}
