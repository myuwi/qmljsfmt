use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use crate::error::{Error, Result};
use crate::indentation::{DEFAULT_INDENT_WIDTH, Indentation};

pub(crate) fn format(source: &str, indentation: Indentation) -> Result<String> {
    let directory = tempfile::tempdir().map_err(|source| Error::OxfmtIo {
        operation: "prepare temporary oxfmt directory",
        source,
    })?;
    let config_path = directory.path().join(".oxfmtrc.json");
    let (tab_width, use_tabs) = match indentation {
        Indentation::Spaces(width) => (width, false),
        Indentation::Tabs => (DEFAULT_INDENT_WIDTH, true),
    };
    let config = format!(r#"{{"tabWidth":{tab_width},"useTabs":{use_tabs}}}"#);

    fs::write(&config_path, config).map_err(|source| Error::OxfmtIo {
        operation: "write oxfmt configuration",
        source,
    })?;

    let output = run(source, &config_path, directory.path()).map_err(|source| Error::OxfmtIo {
        operation: "run oxfmt",
        source,
    })?;

    if !output.status.success() {
        return Err(Error::OxfmtFailed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    String::from_utf8(output.stdout).map_err(Error::InvalidOxfmtOutput)
}

fn run(source: &str, config_path: &Path, directory: &Path) -> io::Result<Output> {
    let mut child = Command::new("oxfmt")
        .arg("--config")
        .arg(config_path)
        .arg("--stdin-filepath")
        .arg("synthetic.ts")
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .expect("oxfmt stdin should be piped")
        .write_all(source.as_bytes())?;

    child.wait_with_output()
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn formats_with_spaces() {
        let source = indoc! {r#"
            function test() {
            if (ready) {
            activate()
            }
            }
        "#};

        assert_eq!(
            format(source, Indentation::Spaces(4)).unwrap(),
            indoc! {r#"
                function test() {
                    if (ready) {
                        activate();
                    }
                }
            "#}
        );
    }

    #[test]
    fn formats_with_tabs() {
        let source = indoc! {r#"
            function test() {
            if (ready) {
            activate()
            }
            }
        "#};

        assert_eq!(
            format(source, Indentation::Tabs).unwrap(),
            "function test() {\n\tif (ready) {\n\t\tactivate();\n\t}\n}\n"
        );
    }

    #[test]
    fn reports_formatter_errors() {
        let error = format("const =", Indentation::default()).unwrap_err();

        assert!(matches!(error, Error::OxfmtFailed { .. }));
    }
}
