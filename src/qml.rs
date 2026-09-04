use crate::error::{Error, Result};

pub(crate) fn parse(input: &str) -> Result<tree_sitter::Tree> {
    let tree = parse_tree(input)?;

    if tree.root_node().has_error() {
        return Err(Error::InvalidQml);
    }

    Ok(tree)
}

pub(crate) fn parse_tree(input: &str) -> Result<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_qmljs::LANGUAGE.into())?;

    parser.parse(input, None).ok_or(Error::ParserFailed)
}
