use lsp_types::{DiagnosticSeverity, DiagnosticTag};

pub const SHELLCHECK_DIALECTS: &[&str] = &["sh", "bash", "dash", "ksh", "busybox"];

#[must_use]
pub fn level_to_severity(level: &str) -> DiagnosticSeverity {
    match level {
        "warning" => DiagnosticSeverity::WARNING,
        "info" => DiagnosticSeverity::INFORMATION,
        "style" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::ERROR,
    }
}

#[must_use]
pub fn code_to_tags(code: u32) -> Option<Vec<DiagnosticTag>> {
    match code {
        2034 => Some(vec![DiagnosticTag::UNNECESSARY]),
        _ => None,
    }
}
