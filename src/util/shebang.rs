pub const BASH_DIALECTS: &[&str] = &["sh", "bash", "dash", "ksh", "zsh", "csh", "ash", "busybox"];

pub struct FileAnalysis {
    pub shebang: Option<String>,
    pub directive: Option<String>,
    pub is_detected: bool,
    pub dialect: Option<String>,
}

#[must_use]
pub fn analyze_file(uri: &str, content: &str) -> FileAnalysis {
    let directive = parse_shell_directive(content);
    let shebang = parse_shebang(content);
    let parsed = directive
        .clone()
        .or_else(|| shebang.clone())
        .or_else(|| parse_uri(uri));
    let dialect = match parsed.as_deref() {
        None => Some("bash".to_string()),
        Some(d) if BASH_DIALECTS.contains(&d) => Some(d.to_string()),
        Some(_) => None,
    };
    let is_detected = shebang.is_some() || directive.is_some();
    FileAnalysis {
        shebang,
        directive,
        is_detected,
        dialect,
    }
}

fn parse_shebang(content: &str) -> Option<String> {
    let line = content.lines().next()?;
    let rest = line.strip_prefix("#!")?;
    let rest = rest.trim();
    shell_from_shebang(rest)
}

fn shell_from_shebang(shebang: &str) -> Option<String> {
    let mut parts = shebang.split_whitespace();
    let path_part = parts.next()?;
    let base = path_part.split('/').next_back()?;

    if base == "env" {
        let next = parts.next()?;
        let next = if next == "-S" || next == "--split-string" {
            parts.next()?
        } else {
            next
        };
        return Some(next.split('/').next_back()?.to_string());
    }
    Some(base.to_string())
}

fn parse_shell_directive(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.contains("shellcheck")) {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#')
            && let Some(inner) = rest.trim().strip_prefix("shellcheck")
        {
            for part in inner.split_whitespace() {
                if let Some(shell) = part.strip_prefix("shell=") {
                    return Some(shell.to_string());
                }
            }
            continue;
        }
        break;
    }
    None
}

fn parse_uri(uri: &str) -> Option<String> {
    if std::path::Path::new(uri)
        .extension()
        .and_then(|e| e.to_str())
        == Some("zsh")
    {
        Some("zsh".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_defaults_to_bash() {
        let fa = analyze_file("", "");
        assert!(fa.shebang.is_none());
        assert!(fa.directive.is_none());
        assert!(!fa.is_detected);
        assert_eq!(fa.dialect.as_deref(), Some("bash"));
    }

    #[test]
    fn python_shebang_returns_no_dialect() {
        let fa = analyze_file("", "#!/usr/bin/env python2.7\n# set -x");
        assert_eq!(fa.shebang.as_deref(), Some("python2.7"));
        assert!(fa.directive.is_none());
        assert!(fa.is_detected);
        assert!(fa.dialect.is_none());
    }

    #[test]
    fn fish_shebang_returns_no_dialect() {
        let fa = analyze_file("", "#!/usr/bin/fish");
        assert_eq!(fa.shebang.as_deref(), Some("fish"));
        assert!(fa.is_detected);
        assert!(fa.dialect.is_none());
    }

    #[test]
    fn shell_shebang_variants() {
        let cases = [
            ("#!/bin/sh -", "sh"),
            ("#!/bin/sh", "sh"),
            ("#!/bin/env sh", "sh"),
            ("#!/usr/bin/env bash", "bash"),
            ("#!/bin/env bash", "bash"),
            ("#!/bin/bash", "bash"),
            ("#!/bin/bash -u", "bash"),
            ("#! /bin/bash", "bash"),
            ("#! /bin/dash", "dash"),
            ("#!/usr/bin/bash", "bash"),
            ("#!/usr/bin/zsh", "zsh"),
        ];
        for (shebang, expected) in cases {
            let fa = analyze_file("", shebang);
            assert_eq!(
                fa.dialect.as_deref(),
                Some(expected),
                "failed for {shebang:?}"
            );
            assert!(fa.is_detected, "is_detected should be true for {shebang:?}");
        }
    }

    #[test]
    fn shellcheck_shell_directive() {
        let fa = analyze_file("", "# shellcheck shell=dash");
        assert!(fa.shebang.is_none());
        assert_eq!(fa.directive.as_deref(), Some("dash"));
        assert!(fa.is_detected);
        assert_eq!(fa.dialect.as_deref(), Some("dash"));
    }

    #[test]
    fn multiple_shellcheck_directives_picks_shell() {
        let fa = analyze_file(
            "",
            "# shellcheck enable=require-variable-braces shell=dash disable=SC1000",
        );
        assert_eq!(fa.directive.as_deref(), Some("dash"));
        assert_eq!(fa.dialect.as_deref(), Some("dash"));
    }

    #[test]
    fn zsh_uri_extension_fallback() {
        let fa = analyze_file("file:///foo/bar.zsh", "");
        assert!(fa.shebang.is_none());
        assert!(!fa.is_detected);
        assert_eq!(fa.dialect.as_deref(), Some("zsh"));
    }
}
