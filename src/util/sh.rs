use anyhow::Result;
use std::process::Command;

const GET_OPTIONS_SH: &str = include_str!("../../scripts/get-options.sh");

#[must_use]
pub fn get_command_options(cmd: &str, word: &str) -> Vec<String> {
    match Command::new("bash")
        .args(["-c", GET_OPTIONS_SH, "--", cmd, word])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .split('\t')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.starts_with('-'))
            .collect(),
        _ => vec![],
    }
}

pub fn get_shell_documentation(word: &str) -> Result<Option<String>> {
    if word.chars().any(|c| c == ' ' || c == '\n' || c == '\t') {
        return Err(anyhow::anyhow!("Invalid word: {word:?}"));
    }
    let output = Command::new("man").args(["-P", "cat", word]).output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            if text.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
        _ => {
            let output = Command::new("bash")
                .args(["-c", "help \"$1\"", "--", word])
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout).into_owned();
                    if text.trim().is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(text))
                    }
                }
                _ => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_word_returns_none() {
        let result = get_shell_documentation("foobar_unknown_xyz_123").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn builtin_exit_returns_documentation() {
        let result = get_shell_documentation("exit").unwrap();
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("exit"), "expected 'exit' in docs: {text:?}");
    }

    #[test]
    fn ls_returns_man_page() {
        let result = get_shell_documentation("ls").unwrap();
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(
            text.contains("list") || text.contains("LIST") || text.contains("List"),
            "expected 'list' in ls man page: {text:?}",
        );
    }

    #[test]
    fn word_with_space_returns_err() {
        let result = get_shell_documentation("ls foo");
        assert!(result.is_err());
    }

    #[test]
    fn word_with_newline_returns_err() {
        let result = get_shell_documentation("ls\nfoo");
        assert!(result.is_err());
    }
}
