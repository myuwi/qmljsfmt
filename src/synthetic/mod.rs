mod decode;
mod encode;

pub(crate) use decode::Replacement;

use crate::error::Result;
use crate::fragments::Fragment;

pub(crate) const DEFAULT_INDENT_WIDTH: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Indentation {
    /// One indent unit, always at least one space wide.
    Spaces(usize),
    Tabs,
}

impl Default for Indentation {
    fn default() -> Self {
        Self::Spaces(DEFAULT_INDENT_WIDTH)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Document {
    pub(crate) source: String,
    pub(crate) indentation: Indentation,
    marker_prefix: String,
}

pub(crate) fn encode(source: &str, tree: &tree_sitter::Tree, fragments: &[Fragment]) -> Document {
    encode::encode(source, tree, fragments)
}

pub(crate) fn decode(
    formatted: &str,
    document: &Document,
    fragments: &[Fragment],
) -> Result<Vec<Replacement>> {
    decode::decode(formatted, &document.marker_prefix, fragments)
}
