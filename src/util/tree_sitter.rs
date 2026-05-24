use lsp_types::{Position, Range};
use tree_sitter::{Node, Point};

#[must_use]
pub fn node_range(node: Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: u32::try_from(start.row).unwrap_or(0),
            character: u32::try_from(start.column).unwrap_or(0),
        },
        end: Position {
            line: u32::try_from(end.row).unwrap_or(0),
            character: u32::try_from(end.column).unwrap_or(0),
        },
    }
}

#[must_use]
pub const fn position_to_point(p: Position) -> Point {
    Point {
        row: p.line as usize,
        column: p.character as usize,
    }
}

#[must_use]
pub fn is_definition(node: Node<'_>) -> bool {
    matches!(node.kind(), "variable_assignment" | "function_definition")
}

#[must_use]
pub fn is_reference(node: Node<'_>) -> bool {
    matches!(node.kind(), "variable_name" | "command_name")
}

#[must_use]
pub fn is_variable_in_read_command(node: Node<'_>, source: &[u8]) -> bool {
    if node.kind() != "word" {
        return false;
    }
    let text = node.utf8_text(source).unwrap_or("");
    if text.starts_with('-') {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "command" {
        return false;
    }
    let Some(first) = parent.child(0) else {
        return false;
    };
    if first.utf8_text(source).unwrap_or("") != "read" {
        return false;
    }
    let prev_text = node
        .prev_sibling()
        .and_then(|s| s.utf8_text(source).ok())
        .unwrap_or("")
        .to_string();
    !regex_is_flag_with_arg(&prev_text)
}

fn regex_is_flag_with_arg(s: &str) -> bool {
    if !s.starts_with('-') {
        return false;
    }
    s.chars().last().is_some_and(|c| "dinNptu".contains(c))
}

#[must_use]
pub fn nodes_same(a: Node<'_>, b: Node<'_>) -> bool {
    a.start_byte() == b.start_byte() && a.end_byte() == b.end_byte()
}

pub fn find_parent<'tree, F>(node: Node<'tree>, predicate: F) -> Option<Node<'tree>>
where
    F: Fn(Node<'tree>) -> bool,
{
    let mut cur = node.parent();
    while let Some(n) = cur {
        if predicate(n) {
            return Some(n);
        }
        cur = n.parent();
    }
    None
}

pub fn collect_typed_nodes<'a>(
    node: Node<'a>,
    kinds: &[&str],
    after: tree_sitter::Point,
    out: &mut Vec<Node<'a>>,
) {
    if node.end_position() < after {
        return;
    }
    if kinds.contains(&node.kind()) && node.start_position() >= after {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_typed_nodes(child, kinds, after, out);
    }
}

pub fn for_each<'tree, F>(node: Node<'tree>, callback: &mut F)
where
    F: FnMut(Node<'tree>) -> bool,
{
    if callback(node) {
        let mut cursor = node.walk();
        let children: Vec<Node<'tree>> = node.children(&mut cursor).collect();
        for child in children {
            for_each(child, callback);
        }
    }
}

#[must_use]
pub fn resolve_static_string(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).unwrap_or("");
    match node.kind() {
        "concatenation" => {
            let mut result = String::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                result.push_str(&resolve_static_string(child, source)?);
            }
            Some(result)
        }
        "word" => Some(text.to_string()),
        "string" | "raw_string" => {
            let mut cursor = node.walk();
            let named: Vec<_> = node.named_children(&mut cursor).collect();
            if named.is_empty() {
                // strip surrounding quotes
                let inner = &text[1..text.len().saturating_sub(1)];
                Some(inner.to_string())
            } else if named.len() == 1 && named[0].kind() == "string_content" {
                named[0]
                    .utf8_text(source)
                    .ok()
                    .map(std::string::ToString::to_string)
            } else {
                None
            }
        }
        _ => None,
    }
}
