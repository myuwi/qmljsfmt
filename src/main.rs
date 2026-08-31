use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    validate_qml(&input)?;
    io::stdout().write_all(input.as_bytes())
}

fn validate_qml(input: &str) -> io::Result<()> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_qmljs::LANGUAGE.into())
        .map_err(io::Error::other)?;

    let tree = parser
        .parse(input, None)
        .ok_or_else(|| io::Error::other("QML parser returned no syntax tree"))?;

    if tree.root_node().has_error() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid QML document",
        ));
    }

    Ok(())
}
