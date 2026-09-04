use std::io;
use std::process::ExitStatus;
use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("failed to configure the QML parser")]
    ParserLanguage(#[from] tree_sitter::LanguageError),

    #[error("QML parser returned no syntax tree")]
    ParserFailed,

    #[error("invalid QML document")]
    InvalidQml,

    #[error("failed to {operation}")]
    OxfmtIo {
        operation: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("oxfmt failed with {status}\n{stderr}")]
    OxfmtFailed { status: ExitStatus, stderr: String },

    #[error("oxfmt produced invalid UTF-8")]
    InvalidOxfmtOutput(#[source] FromUtf8Error),

    #[error("invalid formatted wrapper for fragment {index}")]
    InvalidSyntheticOutput { index: usize },
}

pub type Result<T> = std::result::Result<T, Error>;
