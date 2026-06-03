use std::path::Path;

use crate::shellcheck::directive::{Directive, parse_shellcheck_directive};
use crate::util::fs::{path_to_uri, untildify, uri_to_path, uri_to_path_opt};
use crate::util::tree_sitter::{for_each, node_range, resolve_static_string};
use lsp_types::Range;
use tree_sitter::Tree;

const SOURCING_COMMANDS: &[&str] = &["source", "."];

#[derive(Debug, Clone)]
pub struct SourceCommand {
    pub range: Range,
    pub uri: Option<String>,
    pub error: Option<String>,
}

#[must_use]
pub fn get_source_commands(
    tree: &Tree,
    file_uri: &str,
    workspace_root: Option<&str>,
    source: &[u8],
) -> Vec<SourceCommand> {
    let mut commands = Vec::new();

    let file_dir = uri_to_path_opt(file_uri)
        .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()));

    let root_paths: Vec<String> = [
        file_dir,
        workspace_root.map(std::string::ToString::to_string),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect();

    for_each(tree.root_node(), &mut |node| {
        if node.kind() != "command" {
            return true;
        }
        let Some(info) = get_sourced_path_info(node, source) else {
            return true;
        };
        let uri = info
            .sourced_path
            .as_deref()
            .and_then(|p| resolve_sourced_uri(&root_paths, p));
        commands.push(SourceCommand {
            range: node_range(node),
            uri: uri.clone(),
            error: if uri.is_some() {
                None
            } else {
                Some(
                    info.parse_error
                        .unwrap_or_else(|| "failed to resolve path".to_string()),
                )
            },
        });
        true
    });

    commands
}

struct SourcedPathInfo {
    sourced_path: Option<String>,
    parse_error: Option<String>,
}

fn get_sourced_path_info(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<SourcedPathInfo> {
    let cmd_name = node.named_child(0)?;
    let arg_node = node.named_child(1)?;

    let cmd_text = cmd_name.utf8_text(source).ok()?;
    if cmd_name.kind() != "command_name" || !SOURCING_COMMANDS.contains(&cmd_text) {
        return None;
    }

    // Check for shellcheck directive in preceding comment
    if let Some(prev) = node.prev_sibling()
        && prev.kind() == "comment"
        && prev.utf8_text(source).unwrap_or("").contains("shellcheck")
    {
        let comment_text = prev.utf8_text(source).unwrap_or("");
        let directives = parse_shellcheck_directive(comment_text);

        // Check for source= directive
        for d in &directives {
            if let Directive::Source { path } = d {
                if path == "/dev/null" {
                    return None;
                }
                return Some(SourcedPathInfo {
                    sourced_path: Some(path.clone()),
                    parse_error: None,
                });
            }
        }

        // Check if SC1091 (not-following-source) is disabled
        let sc1091_disabled = directives.iter().any(|d| {
            if let Directive::Disable { rules } = d {
                rules.iter().any(|r| r == "SC1091")
            } else {
                false
            }
        });
        if sc1091_disabled {
            return None;
        }

        // Check for source-path= directive
        for d in &directives {
            if let Directive::SourcePath { path: root } = d
                && root != "SCRIPTDIR"
                && arg_node.kind() == "word"
            {
                let arg = arg_node.utf8_text(source).unwrap_or("");
                return Some(SourcedPathInfo {
                    sourced_path: Some(format!("{root}/{arg}")),
                    parse_error: None,
                });
            }
        }
    }

    // Try to resolve static string
    if let Some(s) = resolve_static_string(arg_node, source) {
        return Some(SourcedPathInfo {
            sourced_path: Some(s),
            parse_error: None,
        });
    }

    // Try to handle leading dynamic section in string: "$VAR/static/suffix"
    if arg_node.kind() == "string" {
        let mut cursor = arg_node.walk();
        let named: Vec<_> = arg_node.named_children(&mut cursor).collect();
        let first_is_expansion = named.first().is_some_and(|n| is_expansion(*n, source));
        let rest_are_static =
            named.len() <= 2 && named.get(1).is_none_or(|n| n.kind() == "string_content");
        if first_is_expansion && rest_are_static {
            let text = arg_node.utf8_text(source).unwrap_or("");
            let inner = &text[1..text.len().saturating_sub(1)];
            let var_text = named[0].utf8_text(source).unwrap_or("");
            if inner.starts_with(&format!("{var_text}/")) {
                return Some(SourcedPathInfo {
                    sourced_path: Some(format!(".{}", &inner[var_text.len()..])),
                    parse_error: None,
                });
            }
        }
    }

    Some(SourcedPathInfo {
        sourced_path: None,
        parse_error: Some("non-constant source not supported".to_string()),
    })
}

fn is_expansion(node: tree_sitter::Node<'_>, _source: &[u8]) -> bool {
    matches!(node.kind(), "expansion" | "simple_expansion")
}

fn resolve_sourced_uri(root_paths: &[String], sourced_path: &str) -> Option<String> {
    let sourced_path = if sourced_path.starts_with('~') {
        untildify(sourced_path)
    } else {
        sourced_path.to_string()
    };

    if sourced_path.starts_with('/') {
        let p = Path::new(&sourced_path);
        if p.exists() {
            return Some(path_to_uri(p));
        }
        return None;
    }

    for root in root_paths {
        let root = uri_to_path(root).to_string_lossy().into_owned();
        let candidate = format!("{root}/{sourced_path}");
        let candidate = Path::new(&candidate);
        if candidate.exists()
            && let Ok(canonical) = candidate.canonicalize()
        {
            return Some(path_to_uri(&canonical));
        }
    }

    None
}
