mod error;
mod fragments;
mod oxfmt;
mod qml;
mod synthetic;

pub use error::{Error, Result};

pub fn format(input: &str) -> Result<String> {
    let tree = qml::parse(input)?;
    if tree.root_node().has_error() {
        return Err(Error::InvalidQml);
    }

    let fragments = fragments::discover(input, &tree);
    if fragments.is_empty() {
        return Ok(input.to_owned());
    }

    let synthetic = synthetic::encode(input, &tree, &fragments);
    let formatted = oxfmt::format(&synthetic.source, synthetic.indentation)?;
    let replacements = synthetic::decode(&formatted, &synthetic)?;

    let mut output = input.to_owned();
    for replacement in replacements.into_iter().rev() {
        output.replace_range(replacement.range, &replacement.text);
    }

    if qml::parse(&output)?.root_node().has_error() {
        return Err(Error::InvalidOutput);
    }

    Ok(output)
}
