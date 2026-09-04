mod error;
mod extract;
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
    let formatted = oxfmt::format(&synthetic.source, synthetic.indentation)?;
    let replacements = extract::extract(&formatted, &synthetic.marker_prefix, &fragments)?;

    let mut output = input.to_owned();
    for replacement in replacements.into_iter().rev() {
        output.replace_range(replacement.range, &replacement.text);
    }

    if qml::parse_tree(&output)?.root_node().has_error() {
        return Err(Error::InvalidOutput);
    }

    Ok(output)
}
