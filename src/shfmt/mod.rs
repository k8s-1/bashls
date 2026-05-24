use std::io::Write as IoWrite;
use std::process::{Command, Stdio};

use crate::config::ShfmtConfig;
use crate::util::fs::uri_to_path_opt;
use anyhow::{Result, anyhow};
use lsp_types::{FormattingOptions, Position, Range, TextEdit};

pub struct Formatter {
    pub executable_path: String,
    pub can_format: bool,
}

impl Formatter {
    #[must_use]
    pub fn new(executable_path: String) -> Self {
        let can_format = match Command::new(&executable_path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
        {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn!(
                    "Shfmt: executable not found at '{executable_path}', formatting disabled"
                );
                false
            }
            _ => true,
        };
        Self {
            executable_path,
            can_format,
        }
    }

    pub fn format(
        &mut self,
        uri: &str,
        content: &str,
        format_options: Option<&FormattingOptions>,
        shfmt_config: &ShfmtConfig,
    ) -> Result<Vec<TextEdit>> {
        if !self.can_format {
            return Ok(vec![]);
        }
        let args = build_args(uri, format_options, shfmt_config);
        match self.run_shfmt(content, &args) {
            Ok(formatted) => {
                let end_line = u32::try_from(content.lines().count()).unwrap_or(0);
                let end_col = content
                    .lines()
                    .last()
                    .map_or(0, |l| u32::try_from(l.len()).unwrap_or(0));
                Ok(vec![TextEdit {
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: end_line,
                            character: end_col,
                        },
                    },
                    new_text: formatted,
                }])
            }
            Err(e) => Err(e),
        }
    }

    fn run_shfmt(&self, content: &str, args: &[String]) -> Result<String> {
        let mut all_args = args.to_vec();
        all_args.push("-".to_string());

        log::debug!("Shfmt: {} {}", self.executable_path, all_args.join(" "));

        let mut proc = Command::new(&self.executable_path)
            .args(&all_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!(e))?;

        if let Some(mut stdin) = proc.stdin.take() {
            stdin.write_all(content.as_bytes())?;
        }

        let output = proc.wait_with_output()?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Err(anyhow!(
                "Shfmt exited with status {}: {}",
                output.status,
                stderr
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn build_args(
    uri: &str,
    format_options: Option<&FormattingOptions>,
    config: &ShfmtConfig,
) -> Vec<String> {
    let mut args = Vec::new();
    let mut active = config.clone();

    if let Some(path) = uri_to_path_opt(uri) {
        args.push(format!("--filename={}", path.to_string_lossy()));
        if !config.ignore_editorconfig
            && let Some(ec) = read_editorconfig(&path.to_string_lossy())
        {
            active = apply_editorconfig(active, ec);
        }
    }

    let indent = format_options
        .filter(|o| o.insert_spaces)
        .map_or(0, |o| o.tab_size);
    args.push(format!("-i={indent}"));

    if active.binary_next_line {
        args.push("-bn".to_string());
    }
    if active.case_indent {
        args.push("-ci".to_string());
    }
    if active.func_next_line {
        args.push("-fn".to_string());
    }
    if active.keep_padding {
        args.push("-kp".to_string());
    }
    if active.simplify_code {
        args.push("-s".to_string());
    }
    if active.space_redirects {
        args.push("-sr".to_string());
    }
    if active.language_dialect != "auto" && !active.language_dialect.is_empty() {
        args.push(format!("-ln={}", active.language_dialect));
    }

    args
}

struct EditorconfigShfmt {
    binary_next_line: Option<bool>,
    case_indent: Option<bool>,
    func_next_line: Option<bool>,
    keep_padding: Option<bool>,
    space_redirects: Option<bool>,
    language_dialect: Option<String>,
}

fn read_editorconfig(path: &str) -> Option<EditorconfigShfmt> {
    let editorconfig_path = find_editorconfig(path)?;
    let content = std::fs::read_to_string(editorconfig_path).ok()?;

    let mut result = EditorconfigShfmt {
        binary_next_line: None,
        case_indent: None,
        func_next_line: None,
        keep_padding: None,
        space_redirects: None,
        language_dialect: None,
    };

    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.contains('*') || {
                let section = line.trim_start_matches('[').trim_end_matches(']');
                path.ends_with(section.trim_matches('*'))
            };
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "binary_next_line" => result.binary_next_line = Some(val == "true"),
                "switch_case_indent" => result.case_indent = Some(val == "true"),
                "function_next_line" => result.func_next_line = Some(val == "true"),
                "keep_padding" => result.keep_padding = Some(val == "true"),
                "space_redirects" => result.space_redirects = Some(val == "true"),
                "shell_variant" => result.language_dialect = Some(val.to_string()),
                _ => {}
            }
        }
    }

    let has_shfmt_config = result.binary_next_line.is_some()
        || result.case_indent.is_some()
        || result.func_next_line.is_some()
        || result.keep_padding.is_some()
        || result.space_redirects.is_some()
        || result.language_dialect.is_some();

    if has_shfmt_config { Some(result) } else { None }
}

fn find_editorconfig(path: &str) -> Option<std::path::PathBuf> {
    let mut dir = std::path::Path::new(path).parent()?;
    loop {
        let candidate = dir.join(".editorconfig");
        if candidate.exists() {
            return Some(candidate);
        }
        let parent = dir.parent()?;
        if parent == dir {
            break;
        }
        dir = parent;
    }
    None
}

fn apply_editorconfig(mut config: ShfmtConfig, ec: EditorconfigShfmt) -> ShfmtConfig {
    if let Some(v) = ec.binary_next_line {
        config.binary_next_line = v;
    }
    if let Some(v) = ec.case_indent {
        config.case_indent = v;
    }
    if let Some(v) = ec.func_next_line {
        config.func_next_line = v;
    }
    if let Some(v) = ec.keep_padding {
        config.keep_padding = v;
    }
    if let Some(v) = ec.space_redirects {
        config.space_redirects = v;
    }
    if let Some(v) = ec.language_dialect {
        config.language_dialect = v;
    }
    config
}
