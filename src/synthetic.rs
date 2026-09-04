use std::fmt::Write;

use crate::fragments::{Fragment, FragmentKind};

pub(crate) const DEFAULT_INDENT_WIDTH: usize = 4;
const MARKER_PREFIX: &str = "__qmljsfmt";

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
    pub(crate) marker_prefix: String,
}

pub(crate) fn build(source: &str, tree: &tree_sitter::Tree, fragments: &[Fragment]) -> Document {
    let indentation = infer_indentation(source, tree);
    let marker_prefix = unique_marker_prefix(source);
    let mut synthetic = String::new();

    for (index, fragment) in fragments.iter().enumerate() {
        if index > 0 {
            synthetic.push('\n');
        }

        let marker = format!("{marker_prefix}_{index}");
        write_fragment(&mut synthetic, source, fragment, &marker, indentation);
    }

    Document {
        source: synthetic,
        indentation,
        marker_prefix,
    }
}

fn write_fragment(
    output: &mut String,
    source: &str,
    fragment: &Fragment,
    marker: &str,
    indentation: Indentation,
) {
    debug_assert!(fragment.qml_depth > 0);

    let qml_indent = indent_depth_at(source, fragment.qml_member_start, indentation)
        .unwrap_or(fragment.qml_depth);

    debug_assert!(qml_indent > 0);

    let fragment_source = &source[fragment.range.clone()];
    let leading_trivia = &source[fragment.replacement_start..fragment.range.start];
    let starts_on_later_line = leading_trivia.contains(['\n', '\r']);
    let payload_indent =
        indent_depth_at(source, fragment.range.start, indentation).unwrap_or(qml_indent);
    let outer_depth = match fragment.kind {
        FragmentKind::BindingBlockContents => qml_indent,
        FragmentKind::BindingStatement if starts_on_later_line => payload_indent.saturating_sub(1),
        _ => qml_indent - 1,
    };

    open_scopes(output, outer_depth, indentation);
    write_indent(output, outer_depth, indentation);
    writeln!(output, "/* {marker}_start */").unwrap();

    match fragment.kind {
        FragmentKind::Expression => {
            // Parentheses keep sequence expressions from splitting the assignment.
            // TODO: Width compensation assumes Oxfmt keeps the value and the synthetic
            // punctuation on one line. If it wraps them, the compensation applies to
            // the wrong line.
            let (open, close, parens_width) = if fragment.parenthesize {
                ("(", ")", 2)
            } else {
                ("", "", 0)
            };
            let scaffold_width = if leading_trivia.contains(['\n', '\r'])
                || fragment_source.contains(['\n', '\r'])
            {
                0
            } else {
                // Reserve columns for same-line synthetic punctuation so the total line
                // width matches QML.
                parens_width + 1
            };

            write_indent(output, outer_depth, indentation);
            writeln!(output, "function {marker}() {{").unwrap();
            write_indent(output, qml_indent, indentation);
            let prefix_width = binding_prefix_width(source, fragment);
            write_assignment_prefix_placeholder(
                output,
                prefix_width.saturating_sub(scaffold_width),
            );
            output.push_str(leading_trivia);
            writeln!(output, "{open}{fragment_source}{close};").unwrap();
            write_indent(output, outer_depth, indentation);
            output.push_str("}\n");
        }
        FragmentKind::BindingBlockContents => {
            write_indent(output, qml_indent, indentation);
            write!(output, "function {marker}() {{").unwrap();
            output.push_str(fragment_source);
            output.push_str("}\n");
        }
        FragmentKind::BindingStatement => {
            write_indent(output, outer_depth, indentation);
            if starts_on_later_line {
                write!(output, "function {marker}() {{").unwrap();
            } else {
                writeln!(output, "function {marker}() {{").unwrap();
                write_indent(output, qml_indent, indentation);
                write_binding_prefix_placeholder(output, binding_prefix_width(source, fragment));
            }
            output.push_str(leading_trivia);
            output.push_str(fragment_source);
            output.push('\n');
            write_indent(output, outer_depth, indentation);
            output.push_str("}\n");
        }
        FragmentKind::FunctionDeclaration => {
            write_indent(output, outer_depth, indentation);
            writeln!(output, "{marker}: {{").unwrap();
            write_indent(output, qml_indent, indentation);
            output.push_str(fragment_source);
            output.push('\n');
            write_indent(output, outer_depth, indentation);
            output.push_str("}\n");
        }
    }

    write_indent(output, outer_depth, indentation);
    writeln!(output, "/* {marker}_end */").unwrap();
    close_scopes(output, outer_depth, indentation);
}

fn open_scopes(output: &mut String, count: usize, indentation: Indentation) {
    for depth in 0..count {
        write_indent(output, depth, indentation);
        output.push_str("{\n");
    }
}

fn close_scopes(output: &mut String, count: usize, indentation: Indentation) {
    for depth in (0..count).rev() {
        write_indent(output, depth, indentation);
        output.push_str("}\n");
    }
}

fn write_indent(output: &mut String, depth: usize, indentation: Indentation) {
    let (character, count) = match indentation {
        Indentation::Tabs => ('\t', depth),
        Indentation::Spaces(width) => (' ', depth * width),
    };

    output.extend(std::iter::repeat_n(character, count));
}

/// Writes a property or label placeholder using the remaining width budget.
fn write_binding_prefix_placeholder(output: &mut String, width: usize) {
    // `_:` is the narrowest form that parses as a property key or a label.
    let underscores = width.saturating_sub(1).max(1);

    output.extend(std::iter::repeat_n('_', underscores));
    output.push(':');
}

/// Writes an assignment placeholder using the remaining width budget.
fn write_assignment_prefix_placeholder(output: &mut String, width: usize) {
    // `_ =` is the narrowest assignment prefix after formatting.
    let underscores = width.saturating_sub(2).max(1);

    output.extend(std::iter::repeat_n('_', underscores));
    output.push_str(" =");
}

fn binding_prefix_width(source: &str, fragment: &Fragment) -> usize {
    column_at(source, fragment.replacement_start)
        .saturating_sub(column_at(source, fragment.qml_member_start))
}

fn column_at(source: &str, byte: usize) -> usize {
    line_prefix(source, byte).chars().count()
}

fn line_prefix(source: &str, byte: usize) -> &str {
    let before = &source[..byte];
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    &before[line_start..]
}

fn unique_marker_prefix(source: &str) -> String {
    let mut prefix = MARKER_PREFIX.to_owned();
    while source.contains(&prefix) {
        prefix.push('_');
    }
    prefix
}

fn infer_indentation(source: &str, tree: &tree_sitter::Tree) -> Indentation {
    let Some(initializer) = root_initializer(tree) else {
        return Indentation::default();
    };

    initializer
        .named_children(&mut initializer.walk())
        .filter(|child| child.kind() != "comment")
        .find_map(|child| indent_unit_at(source, child.start_byte()))
        .unwrap_or_default()
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

fn indent_depth_at(source: &str, byte: usize, indentation: Indentation) -> Option<usize> {
    let prefix = leading_whitespace(source, byte)?;
    let (unit, width) = match indentation {
        Indentation::Tabs => (b'\t', 1),
        Indentation::Spaces(width) => (b' ', width),
    };

    debug_assert!(width > 0);

    (prefix.bytes().all(|byte| byte == unit) && prefix.len() % width == 0)
        .then(|| prefix.len() / width)
}

/// Returns the non-empty whitespace before `byte` on its line.
fn leading_whitespace(source: &str, byte: usize) -> Option<&str> {
    let prefix = line_prefix(source, byte);

    (!prefix.is_empty() && prefix.bytes().all(|byte| matches!(byte, b' ' | b'\t')))
        .then_some(prefix)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    fn document(source: &str) -> Document {
        let tree = crate::qml::parse(source).unwrap();
        let fragments = crate::fragments::discover(source, &tree);
        build(source, &tree, &fragments)
    }

    #[test]
    fn wraps_expression_as_an_equally_wide_property() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: parent.width+1
            }
        "#};

        assert_eq!(
            document(source),
            Document {
                source: indoc! {r#"
                    /* __qmljsfmt_0_start */
                    function __qmljsfmt_0() {
                        ___ = parent.width+1;
                    }
                    /* __qmljsfmt_0_end */
                "#}
                .to_owned(),
                indentation: Indentation::Spaces(4),
                marker_prefix: "__qmljsfmt".to_owned(),
            }
        );
    }

    #[test]
    fn gives_each_fragment_its_own_context() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: parent.width+1
                onClicked: if (ready) activate()
                onPressed: {
                    activate()
                }
                function activate(): void { console.log("active") }
            }
        "#};
        let synthetic = document(source).source;

        let expected = indoc! {r#"
            /* __qmljsfmt_0_start */
            function __qmljsfmt_0() {
                ___ = parent.width+1;
            }
            /* __qmljsfmt_0_end */

            /* __qmljsfmt_1_start */
            function __qmljsfmt_1() {
                _________: if (ready) activate()
            }
            /* __qmljsfmt_1_end */

            {
                /* __qmljsfmt_2_start */
                function __qmljsfmt_2() {
                    activate()
                }
                /* __qmljsfmt_2_end */
            }

            /* __qmljsfmt_3_start */
            __qmljsfmt_3: {
                function activate(): void { console.log("active") }
            }
            /* __qmljsfmt_3_end */
        "#};

        assert_eq!(synthetic, expected);
    }

    #[test]
    fn nests_wrappers_at_the_source_indentation() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                Rectangle {
                    width: parent.width+1
                }
            }
        "#};
        let synthetic = document(source);

        assert_eq!(synthetic.indentation, Indentation::Spaces(4));
        assert_eq!(
            synthetic.source,
            indoc! {r#"
                {
                    /* __qmljsfmt_0_start */
                    function __qmljsfmt_0() {
                        ___ = parent.width+1;
                    }
                    /* __qmljsfmt_0_end */
                }
            "#}
        );
    }

    #[test]
    fn accounts_for_visual_indentation_inside_qml_object_arrays() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property list<Item> things: [
                    Item {
                        width: parent.width+1
                    }
                ]
            }
        "#};
        let synthetic = document(source);

        assert_eq!(
            synthetic.source,
            indoc! {r#"
                {
                    {
                        /* __qmljsfmt_0_start */
                        function __qmljsfmt_0() {
                            ___ = parent.width+1;
                        }
                        /* __qmljsfmt_0_end */
                    }
                }
            "#}
        );
    }

    #[test]
    fn infers_tab_indentation() {
        let source = indoc! {"
            import QtQuick

            Item {
            \twidth: parent.width+1
            }
        "};
        let synthetic = document(source);

        assert_eq!(synthetic.indentation, Indentation::Tabs);
        assert_eq!(
            synthetic.source,
            indoc! {"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                \t___ = parent.width+1;
                }
                /* __qmljsfmt_0_end */
            "}
        );
    }

    #[test]
    fn defaults_when_root_members_are_unindented() {
        let source = indoc! {r#"
            import QtQuick

            Item {
            width: parent.width+1
            }
        "#};
        let synthetic = document(source);

        assert_eq!(synthetic.indentation, Indentation::Spaces(4));
        assert_eq!(
            synthetic.source,
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    ___ = parent.width+1;
                }
                /* __qmljsfmt_0_end */
            "#}
        );
    }

    #[test]
    fn parenthesizes_sequence_expressions_without_changing_line_width() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onClicked: prepare(), activate()
            }
        "#};
        let synthetic = document(source).source;

        assert_eq!(
            synthetic,
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    _____ = (prepare(), activate());
                }
                /* __qmljsfmt_0_end */
            "#}
        );
        assert_eq!(
            line_width(source, "prepare()"),
            line_width(&synthetic, "prepare()")
        );
    }

    #[test]
    fn measures_non_ascii_binding_prefixes_as_characters() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property int café: model.count+1
            }
        "#};
        let synthetic = document(source).source;

        assert_eq!(
            line_width(source, "model.count"),
            line_width(&synthetic, "model.count")
        );
    }

    #[test]
    fn measures_binding_prefixes_when_the_value_starts_on_another_line() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property int total:
                    model.count+1
            }
        "#};
        assert_eq!(
            document(source).source,
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    _________________ =
                        model.count+1;
                }
                /* __qmljsfmt_0_end */
            "#}
        );
    }

    #[test]
    fn measures_only_the_colon_line_when_the_binding_prefix_spans_lines() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width /* a very long explanatory comment
                   that wraps */: parent.width+1
            }
        "#};
        let synthetic = document(source).source;

        assert_eq!(
            synthetic,
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    ______________ = parent.width+1;
                }
                /* __qmljsfmt_0_end */
            "#}
        );
        assert_eq!(
            line_width(source, "parent.width"),
            line_width(&synthetic, "parent.width")
        );
    }

    #[test]
    fn copies_comments_between_the_binding_and_its_value() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: /* keep this */
                    parent.width+1
            }
        "#};

        assert_eq!(
            document(source).source,
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    ____ = /* keep this */
                        parent.width+1;
                }
                /* __qmljsfmt_0_end */
            "#}
        );
    }

    #[test]
    fn avoids_marker_collisions() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property string marker: "__qmljsfmt"
            }
        "#};
        let synthetic = document(source);

        assert_eq!(synthetic.marker_prefix, "__qmljsfmt_");
        assert!(synthetic.source.contains("function __qmljsfmt__0()"));
    }

    #[test]
    fn copies_multiline_template_contents_verbatim() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property string text: `first
              significant whitespace
            last`
            }
        "#};
        assert_eq!(
            document(source).source,
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    ___________________ = `first
                  significant whitespace
                last`;
                }
                /* __qmljsfmt_0_end */
            "#}
        );
    }

    #[test]
    fn delimiters_survive_formatting() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: parent.width+1
            }
        "#};
        let synthetic = document(source);

        assert_eq!(
            crate::oxfmt::format(&synthetic.source, synthetic.indentation).unwrap(),
            indoc! {r#"
                /* __qmljsfmt_0_start */
                function __qmljsfmt_0() {
                    ___ = parent.width + 1;
                }
                /* __qmljsfmt_0_end */
            "#}
        );
    }

    fn line_width(source: &str, needle: &str) -> usize {
        line_containing(source, needle).chars().count()
    }

    fn line_containing<'a>(source: &'a str, needle: &str) -> &'a str {
        source
            .lines()
            .find(|line| line.contains(needle))
            .expect("needle should occur in source")
    }
}
