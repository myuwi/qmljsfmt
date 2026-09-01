mod error;
mod fragments;
mod qml;
mod synthetic;

pub use error::{Error, Result};

pub fn format(input: &str) -> Result<String> {
    let tree = qml::parse(input)?;
    let fragments = fragments::discover(input, &tree);
    let _synthetic = synthetic::build(input, &tree, &fragments);

    Ok(input.to_owned())
}
