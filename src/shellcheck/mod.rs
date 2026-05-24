pub mod config;
pub mod directive;
pub mod types;

use std::collections::HashMap;
use std::io::Write as IoWrite;
use std::process::{Command, Stdio};

use crate::util::fs::uri_to_path_opt;
use crate::util::shebang::analyze_file;
use config::{SHELLCHECK_DIALECTS, code_to_tags, level_to_severity};
use lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, TextEdit, Uri,
    WorkspaceEdit,
};
use serde_json::json;
use types::{ShellCheckComment, ShellCheckReplacement, ShellCheckResult};

pub struct Linter {
    pub executable_path: String,
    pub external_sources: bool,
    pub can_lint: bool,
}

#[derive(Debug, Default)]
pub struct LintingResult {
    pub diagnostics: Vec<Diagnostic>,
    pub code_actions: HashMap<String, CodeAction>,
}

impl Linter {
    #[must_use]
    pub fn new(executable_path: String, external_sources: bool) -> Self {
        Self {
            executable_path,
            external_sources,
            can_lint: true,
        }
    }

    pub fn lint(
        &mut self,
        uri: &str,
        content: &str,
        source_paths: &[String],
        additional_args: &[String],
    ) -> LintingResult {
        if !self.can_lint {
            return LintingResult::default();
        }

        let analysis = analyze_file(uri, content);
        let shell_name = if analysis.shebang.is_some() || analysis.directive.is_some() {
            None
        } else if let Some(ref dialect) = analysis.dialect {
            if SHELLCHECK_DIALECTS.contains(&dialect.as_str()) {
                Some(dialect.clone())
            } else {
                return LintingResult::default();
            }
        } else {
            return LintingResult::default();
        };

        let doc_path = uri_to_path_opt(uri);
        let doc_dir = doc_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()));

        let mut effective_source_paths: Vec<String> = source_paths.to_vec();
        if let Some(dir) = doc_dir {
            effective_source_paths.push(dir);
        }

        match self.run_shellcheck(
            content,
            shell_name.as_deref(),
            &effective_source_paths,
            additional_args,
        ) {
            Ok(result) => map_shellcheck_result(uri, result),
            Err(e) => {
                if e.contains("ENOENT") || e.contains("No such file") {
                    log::warn!(
                        "ShellCheck: disabling linting, executable not found at '{}'",
                        self.executable_path
                    );
                    self.can_lint = false;
                } else {
                    log::error!("ShellCheck error: {e}");
                }
                LintingResult::default()
            }
        }
    }

    fn run_shellcheck(
        &self,
        content: &str,
        shell_name: Option<&str>,
        source_paths: &[String],
        additional_args: &[String],
    ) -> Result<ShellCheckResult, String> {
        let mut args: Vec<String> = vec!["--format=json1".to_string()];
        if self.external_sources {
            args.push("--external-sources".to_string());
        }
        for path in source_paths {
            let p = path.trim();
            if !p.is_empty() {
                args.push(format!("--source-path={p}"));
            }
        }
        args.extend_from_slice(additional_args);

        let user_args = additional_args.join(" ");
        if let Some(shell) = shell_name
            && !user_args.contains("--shell")
            && !user_args.contains("-s ")
        {
            args.insert(0, format!("--shell={shell}"));
        }
        args.push("-".to_string());

        log::debug!("ShellCheck: {} {}", self.executable_path, args.join(" "));

        let mut proc = Command::new(&self.executable_path)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ENOENT: {e}"))?;

        if let Some(mut stdin) = proc.stdin.take() {
            let _ = stdin.write_all(content.as_bytes());
        }

        let output = proc.wait_with_output().map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !stderr.is_empty() {
            log::debug!("ShellCheck stderr: {stderr}");
        }

        serde_json::from_str::<ShellCheckResult>(&stdout)
            .map_err(|e| format!("JSON parse failed: {e}: stdout={stdout}"))
    }
}

fn map_shellcheck_result(uri: &str, result: ShellCheckResult) -> LintingResult {
    let mut diagnostics = Vec::new();
    let mut code_actions = HashMap::new();

    for comment in result.comments {
        let range = Range {
            start: Position {
                line: comment.line.saturating_sub(1),
                character: comment.column.saturating_sub(1),
            },
            end: Position {
                line: comment.end_line.saturating_sub(1),
                character: comment.end_column.saturating_sub(1),
            },
        };

        let id = format!(
            "shellcheck|{}|{}:{}-{}:{}",
            comment.code,
            range.start.line,
            range.start.character,
            range.end.line,
            range.end.character
        );

        let diagnostic = Diagnostic {
            range,
            severity: Some(level_to_severity(comment.level.as_str())),
            code: Some(NumberOrString::String(format!("SC{}", comment.code))),
            code_description: Some(lsp_types::CodeDescription {
                href: format!("https://www.shellcheck.net/wiki/SC{}", comment.code)
                    .parse::<Uri>()
                    .unwrap(),
            }),
            source: Some("shellcheck".to_string()),
            message: comment.message.clone(),
            tags: code_to_tags(comment.code),
            related_information: None,
            data: Some(json!({ "id": id })),
        };

        diagnostics.push(diagnostic.clone());

        if let Some(code_action) = make_code_action(&comment, &[diagnostic], uri) {
            code_actions.insert(id, code_action);
        }
    }

    LintingResult {
        diagnostics,
        code_actions,
    }
}

fn make_code_action(
    comment: &ShellCheckComment,
    diagnostics: &[Diagnostic],
    uri: &str,
) -> Option<CodeAction> {
    let fix = comment.fix.as_ref()?;
    if fix.replacements.is_empty() {
        return None;
    }
    let edits = get_text_edits(&fix.replacements)?;
    let uri_key = uri.parse::<Uri>().ok()?;
    Some(CodeAction {
        title: format!("Apply fix for SC{}", comment.code),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(diagnostics.to_vec()),
        edit: Some(WorkspaceEdit {
            changes: Some(std::collections::HashMap::from([(uri_key, edits)])),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn get_text_edits(replacements: &[ShellCheckReplacement]) -> Option<Vec<TextEdit>> {
    match replacements.len() {
        1 => Some(vec![replacement_to_text_edit(&replacements[0])]),
        2 => Some(vec![
            replacement_to_text_edit(&replacements[1]),
            replacement_to_text_edit(&replacements[0]),
        ]),
        _ => None,
    }
}

fn replacement_to_text_edit(r: &ShellCheckReplacement) -> TextEdit {
    TextEdit {
        range: Range {
            start: Position {
                line: r.line.saturating_sub(1),
                character: r.column.saturating_sub(1),
            },
            end: Position {
                line: r.end_line.saturating_sub(1),
                character: r.end_column.saturating_sub(1),
            },
        },
        new_text: r.replacement.clone(),
    }
}
