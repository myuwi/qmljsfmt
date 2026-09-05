mod decode;
mod encode;

use std::ops::Range;

pub(crate) use decode::Replacement;

use crate::error::Result;
use crate::fragments::Fragment;
use crate::indentation::Indentation;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Document {
    pub(crate) source: String,
    pub(crate) indentation: Indentation,
    marker_prefix: String,
    sections: Vec<Section>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Section {
    kind: SectionKind,
    replacement_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SectionKind {
    Expression { scaffold_parentheses: bool },
    BindingBlockContents,
    BindingStatement,
    FunctionDeclaration,
}

pub(crate) fn encode(source: &str, tree: &tree_sitter::Tree, fragments: &[Fragment]) -> Document {
    encode::encode(source, tree, fragments)
}

pub(crate) fn decode(formatted: &str, document: &Document) -> Result<Vec<Replacement>> {
    decode::decode(formatted, document)
}
