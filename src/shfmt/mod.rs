mod editorconfig;

use editorconfig::{apply_editorconfig, read_editorconfig};
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
        &self,
        uri: &str,
        content: &str,
        format_options: Option<&FormattingOptions>,
        shfmt_config: &ShfmtConfig,
    ) -> Result<Vec<TextEdit>> {
        if !self.can_format {
            return Ok(vec![]);
        }
        let args = build_args(uri, format_options, shfmt_config);
        let formatted = self.run_shfmt(content, &args)?;
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
