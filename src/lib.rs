mod error;
mod fragments;
mod qml;

pub use error::{Error, Result};

pub fn format(input: &str) -> Result<String> {
    let tree = qml::parse(input)?;
    let _fragments = fragments::discover(&tree);

    Ok(input.to_owned())
}
