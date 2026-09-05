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

pub(crate) fn infer(source: &str, tree: &tree_sitter::Tree) -> Indentation {
    let Some(initializer) = root_initializer(tree) else {
        return Indentation::default();
    };

    initializer
        .named_children(&mut initializer.walk())
        .filter(|child| child.kind() != "comment")
        .find_map(|child| indent_unit_at(source, child.start_byte()))
        .unwrap_or_default()
}

pub(crate) fn depth_at(source: &str, byte: usize, indentation: Indentation) -> Option<usize> {
    let prefix = leading_whitespace(source, byte)?;
    let (unit, width) = match indentation {
        Indentation::Tabs => (b'\t', 1),
        Indentation::Spaces(width) => (b' ', width),
    };

    debug_assert!(width > 0);

    (prefix.bytes().all(|byte| byte == unit) && prefix.len() % width == 0)
        .then(|| prefix.len() / width)
}

pub(crate) fn column_at(source: &str, byte: usize) -> usize {
    line_prefix(source, byte).chars().count()
}

fn root_initializer(tree: &tree_sitter::Tree) -> Option<tree_sitter::Node<'_>> {
    let root = tree.root_node().child_by_field_name("root")?;
    let definition = if root.kind() == "ui_annotated_object" {
        root.child_by_field_name("definition")?
    } else {
        root
    };
    definition.child_by_field_name("initializer")
}

/// One indent unit, measured from a line known to sit exactly one level deep.
fn indent_unit_at(source: &str, byte: usize) -> Option<Indentation> {
    let prefix = leading_whitespace(source, byte)?;

    match prefix.as_bytes() {
        [b'\t'] => Some(Indentation::Tabs),
        spaces if spaces.iter().all(|byte| *byte == b' ') => {
            Some(Indentation::Spaces(spaces.len()))
        }
        _ => None,
    }
}

/// Returns the non-empty whitespace before `byte` on its line.
fn leading_whitespace(source: &str, byte: usize) -> Option<&str> {
    let prefix = line_prefix(source, byte);

    (!prefix.is_empty() && prefix.bytes().all(|byte| matches!(byte, b' ' | b'\t')))
        .then_some(prefix)
}

fn line_prefix(source: &str, byte: usize) -> &str {
    let before = &source[..byte];
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    &before[line_start..]
}
