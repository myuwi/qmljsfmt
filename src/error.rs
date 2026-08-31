use std::io;

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
}

pub type Result<T> = std::result::Result<T, Error>;
