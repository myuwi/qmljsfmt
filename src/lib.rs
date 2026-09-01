mod error;
mod fragments;
mod oxfmt;
mod qml;
mod synthetic;

pub use error::{Error, Result};

pub fn format(input: &str) -> Result<String> {
    let tree = qml::parse(input)?;
    let fragments = fragments::discover(input, &tree);
    if fragments.is_empty() {
        return Ok(input.to_owned());
    }

    let synthetic = synthetic::build(input, &tree, &fragments);
    let _formatted = oxfmt::format(&synthetic.source, synthetic.indentation)?;

    Ok(input.to_owned())
}
