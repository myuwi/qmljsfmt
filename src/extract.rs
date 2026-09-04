use std::ops::Range;

use tree_sitter::Node;

use crate::error::{Error, Result};
use crate::fragments::{Fragment, FragmentKind};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Replacement {
    pub(crate) range: Range<usize>,
    pub(crate) text: String,
}

/// Each marked region of Oxfmt's output is a valid QML object member, so they are
/// collected into one throwaway QML document and the QML grammar reports the payload
/// boundaries exactly.
pub(crate) fn extract(
    formatted: &str,
    marker_prefix: &str,
    fragments: &[Fragment],
) -> Result<Vec<Replacement>> {
    let members = find_marked_members(formatted, marker_prefix, fragments.len());
    let (document, member_spans) = collect_as_qml(members)?;
    let tree = crate::qml::parse_tree(&document)?;
    let root = tree.root_node();

    let replacements = fragments
        .iter()
        .zip(&member_spans)
        .enumerate()
        .map(|(index, (fragment, span))| {
            let text = member_at(root, span)
                .and_then(|node| fragment_text(&document, node, fragment))
                .ok_or_else(|| invalid_output(index))?;

            Ok(Replacement {
                range: fragment.replacement_start..fragment.range.end,
                text,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    debug_assert!(
        replacements
            .windows(2)
            .all(|pair| pair[0].range.end <= pair[1].range.start)
    );
    Ok(replacements)
}

fn find_marked_members<'a>(
    formatted: &'a str,
    marker_prefix: &'a str,
    marker_count: usize,
) -> impl Iterator<Item = Result<&'a str>> {
    // Members appear in order and never overlap, so each search resumes where the last ended.
    let mut cursor = 0;

    (0..marker_count).map(move |index| {
        let opening = format!("/* {marker_prefix}_{index}_start */");
        let closing = format!("/* {marker_prefix}_{index}_end */");
        let start = formatted[cursor..]
            .find(&opening)
            .ok_or_else(|| invalid_output(index))?
            + cursor
            + opening.len();
        let end = formatted[start..]
            .find(&closing)
            .ok_or_else(|| invalid_output(index))?
            + start;

        cursor = end;
        Ok(formatted[start..end].trim())
    })
}

fn collect_as_qml<'a>(
    members: impl Iterator<Item = Result<&'a str>>,
) -> Result<(String, Vec<Range<usize>>)> {
    let mut document = String::from("Item {\n");
    let mut member_spans = Vec::new();

    for member in members {
        let start = document.len();

        document.push_str(member?);
        member_spans.push(start..document.len());
        document.push('\n');
    }
    document.push('}');

    Ok((document, member_spans))
}

fn member_at<'a>(root: Node<'a>, span: &Range<usize>) -> Option<Node<'a>> {
    let node = root.descendant_for_byte_range(span.start, span.end)?;

    // The descent only ever widens, so an exact match proves the span is a single node.
    (node.byte_range() == *span && !node.has_error()).then_some(node)
}

fn fragment_text(source: &str, member: Node<'_>, fragment: &Fragment) -> Option<String> {
    match fragment.kind {
        FragmentKind::Expression => expression_text(source, member, fragment),
        FragmentKind::BindingStatement => statement_text(source, member),
        FragmentKind::BindingBlockContents => {
            let region = function_body(member).and_then(block_inner)?;
            Some(source[region].to_owned())
        }
        FragmentKind::FunctionDeclaration => {
            let region = binding_block(member).and_then(block_inner)?;
            Some(source[region].trim().to_owned())
        }
    }
}

fn expression_text(source: &str, member: Node<'_>, fragment: &Fragment) -> Option<String> {
    let body = function_body(member)?;
    let region = block_inner(body)?;
    let statement = first_non_trivia(body).filter(|node| node.kind() == "expression_statement")?;
    let assignment = statement
        .named_child(0)
        .filter(|node| node.kind() == "assignment_expression")?;
    let placeholder = assignment
        .child_by_field_name("left")
        .filter(|node| is_placeholder(source, *node))?;
    let equals = child_of_kind(assignment, "=")?;

    let value = assignment.child_by_field_name("right")?;
    let (payload, parens) = match fragment.parenthesize {
        true => (first_non_trivia(value)?, Some(parentheses(value)?)),
        false => (value, None),
    };

    let mut edits = vec![drop_scaffold(source, &region, placeholder, equals)];

    if let Some((open, _)) = parens {
        edits.push(Edit::Delete(open.byte_range()));
    }
    // QML reads a leading `{` after the binding colon as a binding block.
    if let Some(object) = leading_object(payload) {
        edits.push(Edit::Insert {
            at: object.start_byte(),
            text: "(",
        });
        edits.push(Edit::Insert {
            at: object.end_byte(),
            text: ")",
        });
    }
    if let Some((_, close)) = parens {
        edits.push(Edit::Delete(close.byte_range()));
    }
    if let Some(semicolon) = child_of_kind(statement, ";") {
        edits.push(Edit::Delete(semicolon.byte_range()));
    }

    Some(splice(source, region, &edits).trim_end().to_owned())
}

fn statement_text(source: &str, member: Node<'_>) -> Option<String> {
    let body = function_body(member)?;
    let region = block_inner(body)?;
    let mut edits = Vec::new();

    if let Some(labeled) = first_non_trivia(body).filter(|node| node.kind() == "labeled_statement")
    {
        let label = labeled.child_by_field_name("label")?;

        if is_placeholder(source, label) {
            let colon = child_of_kind(labeled, ":")?;
            edits.push(drop_scaffold(source, &region, label, colon));
        }
    }

    Some(splice(source, region, &edits).trim_end().to_owned())
}

enum Edit {
    Delete(Range<usize>),
    Insert { at: usize, text: &'static str },
}

/// Keeps any comment Oxfmt hoisted above the placeholder.
fn drop_scaffold(
    source: &str,
    region: &Range<usize>,
    placeholder: Node<'_>,
    separator: Node<'_>,
) -> Edit {
    if source[region.start..placeholder.start_byte()]
        .trim()
        .is_empty()
    {
        Edit::Delete(region.start..separator.end_byte())
    } else {
        Edit::Delete(placeholder.start_byte()..skip_blanks(source, separator.end_byte()))
    }
}

/// Nothing is mutated in place, so every offset stays a `source` coordinate.
fn splice(source: &str, region: Range<usize>, edits: &[Edit]) -> String {
    let mut spliced = String::new();
    let mut cursor = region.start;

    for edit in edits {
        let (span, text) = match edit {
            Edit::Delete(range) => (range.clone(), ""),
            Edit::Insert { at, text } => (*at..*at, *text),
        };
        debug_assert!(
            span.start >= cursor,
            "edits must not overlap or go backwards"
        );

        spliced.push_str(&source[cursor..span.start.max(cursor)]);
        spliced.push_str(text);
        cursor = span.end.max(cursor);
    }
    spliced.push_str(&source[cursor..region.end]);

    spliced
}

fn function_body(member: Node<'_>) -> Option<Node<'_>> {
    if member.kind() != "function_declaration" {
        return None;
    }

    member.child_by_field_name("body")
}

fn binding_block(member: Node<'_>) -> Option<Node<'_>> {
    if member.kind() != "ui_binding" {
        return None;
    }

    let value = member.child_by_field_name("value")?;

    (value.kind() == "statement_block").then_some(value)
}

fn block_inner(block: Node<'_>) -> Option<Range<usize>> {
    let open = block.child(0)?;
    let close = block.child(block.child_count().checked_sub(1)?)?;

    (open.kind() == "{" && close.kind() == "}").then(|| open.end_byte()..close.start_byte())
}

fn first_non_trivia(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();

    node.named_children(&mut cursor)
        .find(|child| child.kind() != "comment")
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();

    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn parentheses(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    if node.kind() != "parenthesized_expression" {
        return None;
    }

    let open = node.child(0)?;
    let close = node.child(node.child_count().checked_sub(1)?)?;

    (open.kind() == "(" && close.kind() == ")").then_some((open, close))
}

fn leading_object(value: Node<'_>) -> Option<Node<'_>> {
    let mut node = value;

    while node.kind() != "object" {
        let child = node.named_child(0)?;

        if child.start_byte() != node.start_byte() {
            return None;
        }
        node = child;
    }

    Some(node)
}

fn is_placeholder(source: &str, node: Node<'_>) -> bool {
    let text = &source[node.byte_range()];

    !text.is_empty() && text.bytes().all(|byte| byte == b'_')
}

fn skip_blanks(source: &str, offset: usize) -> usize {
    offset
        + source[offset..]
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count()
}

fn invalid_output(index: usize) -> Error {
    Error::InvalidSyntheticOutput { index }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    fn extracted(source: &str) -> Vec<(String, String)> {
        let tree = crate::qml::parse(source).unwrap();
        let fragments = crate::fragments::discover(source, &tree);
        let synthetic = crate::synthetic::build(source, &tree, &fragments);
        let formatted = crate::oxfmt::format(&synthetic.source, synthetic.indentation).unwrap();

        extract(&formatted, &synthetic.marker_prefix, &fragments)
            .unwrap()
            .into_iter()
            .map(|replacement| {
                (
                    source[replacement.range.clone()].to_owned(),
                    replacement.text,
                )
            })
            .collect()
    }

    fn extracted_source(source: &str) -> String {
        let tree = crate::qml::parse(source).unwrap();
        let fragments = crate::fragments::discover(source, &tree);
        let synthetic = crate::synthetic::build(source, &tree, &fragments);
        let formatted = crate::oxfmt::format(&synthetic.source, synthetic.indentation).unwrap();
        let replacements = extract(&formatted, &synthetic.marker_prefix, &fragments).unwrap();
        let mut output = source.to_owned();

        for replacement in replacements.into_iter().rev() {
            output.replace_range(replacement.range, &replacement.text);
        }
        output
    }

    #[test]
    fn extracts_every_fragment_kind() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: parent.width+1
                onClicked: if (ready) activate()
                onPressed: { let x=1; activate(x) }
                function calculate(value: real): real { return value+1 }
            }
        "#};

        assert_eq!(
            extracted(source),
            vec![
                (" parent.width+1".into(), " parent.width + 1".into()),
                (
                    " if (ready) activate()".into(),
                    " if (ready) activate();".into()
                ),
                (
                    " let x=1; activate(x) ".into(),
                    "\n        let x = 1;\n        activate(x);\n    ".into()
                ),
                (
                    "function calculate(value: real): real { return value+1 }".into(),
                    "function calculate(value: real): real {\n        return value + 1;\n    }"
                        .into()
                ),
            ]
        );
    }

    #[test]
    fn strips_sequence_expression_parentheses() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onClicked: prepare(),activate()
            }
        "#};

        assert_eq!(
            extracted(source),
            vec![(
                " prepare(),activate()".into(),
                " prepare(), activate()".into()
            )]
        );
    }

    #[test]
    fn extracts_wrapped_sequence_expression() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onClicked: first.reallyLongProperty.reallyLongMethod(), second.reallyLongProperty.reallyLongMethod(), third.reallyLongProperty.reallyLongMethod()
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    onClicked:
                        first.reallyLongProperty.reallyLongMethod(),
                        second.reallyLongProperty.reallyLongMethod(),
                        third.reallyLongProperty.reallyLongMethod()
                }
            "#}
        );
    }

    #[test]
    fn strips_sequence_parentheses_without_removing_trivia() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onClicked: /* keep this */
                    prepare(),activate()
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    onClicked: /* keep this */ prepare(), activate()
                }
            "#}
        );
    }

    #[test]
    fn preserves_commas_in_literals_and_comments() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property string quoted: "a, //"
                property string templated: `a, //`
                property int count: /* a, // b */ model.count+1
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    property string quoted: "a, //"
                    property string templated: `a, //`
                    property int count: /* a, // b */ model.count + 1
                }
            "#}
        );
    }

    #[test]
    fn preserves_parentheses_around_object_literals() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property var config: ({a:1,b:2})
                property var member: ({a:1}).a
                property var guarded: ({a:1}) || fallback
                property var chosen: ({a:1}) ? 1 : 2
                onClicked: ({a:1}), activate()
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    property var config: ({ a: 1, b: 2 })
                    property var member: ({ a: 1 }).a
                    property var guarded: ({ a: 1 }) || fallback
                    property var chosen: ({ a: 1 }) ? 1 : 2
                    onClicked: ({ a: 1 }), activate()
                }
            "#}
        );
    }

    #[test]
    fn keeps_comments_oxfmt_moves_past_the_scaffold() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: (parent.width /* trailing */)
                onClicked: (prepare(), activate() /* trailing */)
                property var handler: function () { return 1 } /* trailing */
                onPressed: { activate() } /* trailing */
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    width: parent.width /* trailing */
                    onClicked: (prepare(), activate() /* trailing */)
                    property var handler: function () {
                        return 1;
                    } /* trailing */
                    onPressed: {
                        activate();
                    } /* trailing */
                }
            "#}
        );
    }

    #[test]
    fn preserves_labels_in_later_line_statements() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onClicked:
                    if (ready) {
                        _: activate()
                    }
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    onClicked:
                        if (ready) {
                            _: activate();
                        }
                }
            "#}
        );
    }

    #[test]
    fn formats_leading_trivia_with_the_fragment() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width: /* explanation */
                    parent.width+1
                onClicked: // prepare
                    if (ready) { activate() }
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    width: /* explanation */ parent.width + 1
                    onClicked:
                        // prepare
                        if (ready) {
                            activate();
                        }
                }
            "#}
        );
    }

    #[test]
    fn preserves_multiline_template_contents() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                text: `first
              second ${value+1}`
            }
        "#};

        assert_eq!(
            extracted_source(source),
            indoc! {r#"
                import QtQuick

                Item {
                    text: `first
                  second ${value + 1}`
                }
            "#}
        );
    }

    #[test]
    fn rejects_malformed_sections() {
        let source = "Item {\n    width: parent.width\n}\n";
        let tree = crate::qml::parse(source).unwrap();
        let fragments = crate::fragments::discover(source, &tree);

        let missing = extract("const unrelated = 1;\n", "__qmljsfmt", &fragments).unwrap_err();
        assert!(matches!(
            missing,
            Error::InvalidSyntheticOutput { index: 0 }
        ));

        let marker = "__qmljsfmt_0";
        let start = format!("/* {marker}_start */");
        let duplicated = format!("{start}\n{start}\n");
        let duplicate = extract(&duplicated, "__qmljsfmt", &fragments).unwrap_err();
        assert!(matches!(
            duplicate,
            Error::InvalidSyntheticOutput { index: 0 }
        ));

        let end = format!("/* {marker}_end */");
        let wrong_wrapper = format!("{start}\nconst {marker} = [];\n{end}\n");
        let malformed = extract(&wrong_wrapper, "__qmljsfmt", &fragments).unwrap_err();
        assert!(matches!(
            malformed,
            Error::InvalidSyntheticOutput { index: 0 }
        ));
    }

    fn marked_members(formatted: &str, count: usize) -> Result<Vec<&str>> {
        find_marked_members(formatted, "m", count).collect()
    }

    #[test]
    fn finds_every_marked_member_in_order() {
        let formatted = concat!(
            "/* m_0_start */ a /* m_0_end */\n",
            "/* m_1_start */ b /* m_1_end */\n",
        );

        assert_eq!(marked_members(formatted, 2).unwrap(), vec!["a", "b"]);
        assert_eq!(marked_members(formatted, 1).unwrap(), vec!["a"]);

        assert!(marked_members(formatted, 3).is_err());
        assert!(marked_members("/* m_0_start */", 1).is_err());
        assert!(marked_members("/* m_0_end */ /* m_0_start */", 1).is_err());
    }

    #[test]
    fn collects_members_into_one_object() {
        let members = ["a", "b"].map(Ok);
        let (document, member_spans) = collect_as_qml(members.into_iter()).unwrap();

        assert_eq!(document, "Item {\na\nb\n}");
        assert_eq!(
            member_spans
                .iter()
                .map(|span| &document[span.clone()])
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn splices_deletions_and_insertions() {
        let source = "0123456789";

        assert_eq!(splice(source, 2..8, &[]), "234567");
        assert_eq!(splice(source, 2..8, &[Edit::Delete(4..6)]), "2367");
        assert_eq!(
            splice(source, 2..8, &[Edit::Insert { at: 4, text: "(" }]),
            "23(4567"
        );
        assert_eq!(
            splice(
                source,
                2..8,
                &[
                    Edit::Insert { at: 4, text: "(" },
                    Edit::Insert { at: 6, text: ")" }
                ]
            ),
            "23(45)67"
        );
        assert_eq!(
            splice(source, 2..8, &[Edit::Delete(2..3), Edit::Delete(7..8)]),
            "3456"
        );
    }

    /// The insertion that closes a leading object literal shares its offset with the
    /// scaffold semicolon whenever the whole value is the object.
    #[test]
    fn splices_an_insertion_abutting_a_deletion() {
        let source = "___ = { a: 1 };";

        assert_eq!(
            splice(
                source,
                0..source.len(),
                &[
                    Edit::Delete(0..5),
                    Edit::Insert { at: 6, text: "(" },
                    Edit::Insert { at: 14, text: ")" },
                    Edit::Delete(14..15),
                ]
            ),
            " ({ a: 1 })"
        );
    }
}
