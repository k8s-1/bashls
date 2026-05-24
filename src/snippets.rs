use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, MarkupContent, MarkupKind};
use serde_json::json;

#[must_use]
pub fn get_snippets() -> Vec<CompletionItem> {
    SNIPPET_DATA
        .iter()
        .map(|(label, doc, insert_text)| {
            let documentation =
                format!("```man\n{doc} (bash-language-server)\n\n```\n```bash\n{insert_text}\n```");
            CompletionItem {
                label: label.to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                insert_text: Some(insert_text.to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                documentation: Some(lsp_types::Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: documentation,
                })),
                data: Some(json!({ "type": 4 })),
                ..Default::default()
            }
        })
        .collect()
}

const SNIPPET_DATA: &[(&str, &str, &str)] = &[
    ("shebang", "shebang", "#!/usr/bin/env ${1|bash,sh|}"),
    (
        "shebang-with-arguments",
        "shebang-with-arguments",
        "#!/usr/bin/env ${1|-S,--split-string|} ${2|bash,sh|} ${3|argument ...|}",
    ),
    (
        "and",
        "and operator",
        "${1:first-expression} && ${2:second-expression}",
    ),
    (
        "or",
        "or operator",
        "${1:first-expression} || ${2:second-expression}",
    ),
    (
        "if",
        "if operator",
        "if ${1:condition}; then\n\t${2:command ...}\nfi",
    ),
    (
        "if-else",
        "if-else operator",
        "if ${1:condition}; then\n\t${2:command ...}\nelse\n\t${3:command ...}\nfi",
    ),
    (
        "if-less",
        "if with number comparison",
        "if (( \"${1:first-expression}\" < \"${2:second-expression}\" )); then\n\t${3:command ...}\nfi",
    ),
    (
        "if-greater",
        "if with number comparison",
        "if (( \"${1:first-expression}\" > \"${2:second-expression}\" )); then\n\t${3:command ...}\nfi",
    ),
    (
        "if-less-or-equal",
        "if with number comparison",
        "if (( \"${1:first-expression}\" <= \"${2:second-expression}\" )); then\n\t${3:command ...}\nfi",
    ),
    (
        "if-greater-or-equal",
        "if with number comparison",
        "if (( \"${1:first-expression}\" >= \"${2:second-expression}\" )); then\n\t${3:command ...}\nfi",
    ),
    (
        "if-equal",
        "if with number comparison",
        "if (( \"${1:first-expression}\" == \"${2:second-expression}\" )); then\n\t${3:command ...}\nfi",
    ),
    (
        "if-not-equal",
        "if with number comparison",
        "if (( \"${1:first-expression}\" != \"${2:second-expression}\" )); then\n\t${3:command ...}\nfi",
    ),
    (
        "if-string-equal",
        "if with string comparison",
        "if [[ \"${1:first-expression}\" == \"${2:second-expression}\" ]]; then\n\t${3:command ...}\nfi",
    ),
    (
        "if-string-not-equal",
        "if with string comparison",
        "if [[ \"${1:first-expression}\" != \"${2:second-expression}\" ]]; then\n\t${3:command ...}\nfi",
    ),
    (
        "if-string-empty",
        "if with string comparison (has [z]ero length)",
        "if [[ -z \"${1:expression}\" ]]; then\n\t${2:command ...}\nfi",
    ),
    (
        "if-string-not-empty",
        "if with string comparison ([n]ot empty)",
        "if [[ -n \"${1:expression}\" ]]; then\n\t${2:command ...}\nfi",
    ),
    (
        "if-defined",
        "if with variable existence check",
        "if [[ -n \"\\${${1:variable}+x}\" ]]; then\n\t${2:command ...}\nfi",
    ),
    (
        "if-not-defined",
        "if with variable existence check",
        "if [[ -z \"\\${${1:variable}+x}\" ]]; then\n\t${2:command ...}\nfi",
    ),
    (
        "while",
        "while operator",
        "while ${1:condition}; do\n\t${2:command ...}\ndone",
    ),
    (
        "while-less",
        "while with number comparison",
        "while (( \"${1:first-expression}\" < \"${2:second-expression}\" )); do\n\t${3:command ...}\ndone",
    ),
    (
        "while-greater",
        "while with number comparison",
        "while (( \"${1:first-expression}\" > \"${2:second-expression}\" )); do\n\t${3:command ...}\ndone",
    ),
    (
        "while-string-equal",
        "while with string comparison",
        "while [[ \"${1:first-expression}\" == \"${2:second-expression}\" ]]; do\n\t${3:command ...}\ndone",
    ),
    (
        "while-string-empty",
        "while with string comparison (has [z]ero length)",
        "while [[ -z \"${1:expression}\" ]]; do\n\t${2:command ...}\ndone",
    ),
    (
        "until",
        "until operator",
        "until ${1:condition}; do\n\t${2:command ...}\ndone",
    ),
    (
        "for",
        "for operator",
        "for ${1:item} in ${2:expression}; do\n\t${3:command ...}\ndone",
    ),
    (
        "for-range",
        "for with range",
        "for ${1:item} in \\$(seq ${2:from} ${3:to}); do\n\t${4:command ...}\ndone",
    ),
    (
        "for-files",
        "for with files",
        "for ${1:item} in *.${2:extension}; do\n\t${4:command ...}\ndone",
    ),
    (
        "case",
        "case operator",
        "case \"${1:expression}\" in\n\t${2:pattern})\n\t\t${3:command ...}\n\t\t;;\n\t*)\n\t\t${4:command ...}\n\t\t;;\nesac",
    ),
    (
        "function",
        "function definition",
        "${1:name}() {\n\t${2:command ...}\n}",
    ),
    (
        "documentation",
        "documentation definition",
        "# ${1:function_name} ${2:function_parameters}\n# ${3:function_description}\n#\n# Output:\n#   ${4:function_output}\n#\n# Return:\n# - ${5:0} when ${6:all parameters are correct}\n# - ${7:1} ${8:otherwise}",
    ),
    ("block", "block", "{\n\t${1:command ...}\n}"),
    (
        "block-redirected",
        "block redirected",
        "{\n\t${1:command ...}\n} > ${2:file}",
    ),
    ("variable", "variable", "declare ${1:variable}=${2:value}"),
    (
        "if-unset-or-null",
        "if unset or null",
        "\"\\${${1:variable}:-${2:default}}\"",
    ),
    (
        "if-unset",
        "if unset",
        "\"\\${${1:variable}-${2:default}}\"",
    ),
    (
        "set-if-unset-or-null",
        "set if unset or null",
        "\"\\${${1:variable}:=${2:default}}\"",
    ),
    ("comment definition", "comment", "# ${1:description}"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn snippet_labels_are_unique() {
        let snippets = get_snippets();
        let labels: Vec<&str> = snippets.iter().map(|s| s.label.as_str()).collect();
        let unique: HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(
            labels.len(),
            unique.len(),
            "snippet labels should be unique"
        );
    }

    #[test]
    fn all_snippets_have_insert_text() {
        for s in get_snippets() {
            assert!(
                s.insert_text.is_some(),
                "snippet '{}' missing insert_text",
                s.label
            );
        }
    }

    #[test]
    fn all_snippets_are_snippet_kind() {
        for s in get_snippets() {
            assert_eq!(s.kind, Some(lsp_types::CompletionItemKind::SNIPPET));
        }
    }
}
