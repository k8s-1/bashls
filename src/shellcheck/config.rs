use lsp_types::{DiagnosticSeverity, DiagnosticTag};

use super::types::ShellCheckLevel;

pub const SHELLCHECK_DIALECTS: &[&str] = &["sh", "bash", "dash", "ksh", "busybox"];

#[must_use]
pub fn level_to_severity(level: ShellCheckLevel) -> DiagnosticSeverity {
    match level {
        ShellCheckLevel::Warning => DiagnosticSeverity::WARNING,
        ShellCheckLevel::Info => DiagnosticSeverity::INFORMATION,
        ShellCheckLevel::Style => DiagnosticSeverity::HINT,
        ShellCheckLevel::Error => DiagnosticSeverity::ERROR,
    }
}

#[must_use]
pub fn code_to_tags(code: u32) -> Option<Vec<DiagnosticTag>> {
    match code {
        2034 => Some(vec![DiagnosticTag::UNNECESSARY]),
        _ => None,
    }
}
