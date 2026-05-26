#[derive(Debug, Clone)]
pub enum Directive {
    Disable { rules: Vec<String> },
    Source { path: String },
    SourcePath { path: String },
}

#[must_use]
pub fn parse_shellcheck_directive(line: &str) -> Vec<Directive> {
    let Some(rest) = find_directive_rest(line) else {
        return vec![];
    };

    rest.split_whitespace()
        .filter_map(|command| {
            let (type_key, value) = command.split_once('=')?;
            match type_key {
                "source" => Some(Directive::Source {
                    path: value.to_string(),
                }),
                "source-path" => Some(Directive::SourcePath {
                    path: value.to_string(),
                }),
                "disable" => Some(Directive::Disable {
                    rules: parse_rules(value),
                }),
                _ => None,
            }
        })
        .collect()
}

fn find_directive_rest(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('#')?.trim();
    let rest = rest.strip_prefix("shellcheck")?;
    Some(rest.trim())
}

fn parse_rules(value: &str) -> Vec<String> {
    let mut rules = Vec::new();
    for arg in value.split(',') {
        let arg = arg.trim();
        if arg.is_empty() {
            continue;
        }
        if let Some(range_match) = parse_sc_range(arg) {
            rules.extend(range_match);
        } else {
            rules.push(arg.to_string());
        }
    }
    rules
}

fn parse_sc_range(s: &str) -> Option<Vec<String>> {
    let (start_s, end_s) = s.split_once('-')?;
    let start_num: u32 = start_s.strip_prefix("SC")?.parse().ok()?;
    let end_num: u32 = end_s.strip_prefix("SC")?.parse().ok()?;
    Some((start_num..=end_num).map(|i| format!("SC{i}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(d: &Directive) -> &[String] {
        match d {
            Directive::Disable { rules } => rules,
            _ => panic!("not Disable"),
        }
    }
    fn path(d: &Directive) -> &str {
        match d {
            Directive::Source { path } | Directive::SourcePath { path } => path,
            _ => panic!("not Source/SourcePath"),
        }
    }

    #[test]
    fn disable_single() {
        let d = parse_shellcheck_directive("# shellcheck disable=SC1000");
        assert_eq!(d.len(), 1);
        assert_eq!(rules(&d[0]), &["SC1000"]);
    }

    #[test]
    fn disable_comma_separated() {
        let d = parse_shellcheck_directive("# shellcheck disable=SC1000,SC1001");
        assert_eq!(d.len(), 1);
        assert_eq!(rules(&d[0]), &["SC1000", "SC1001"]);
    }

    #[test]
    fn disable_range_with_mixed() {
        let d = parse_shellcheck_directive(
            "# shellcheck disable=SC1000,SC2000-SC2002,SC1001 # this is a comment",
        );
        assert_eq!(d.len(), 1);
        assert_eq!(
            rules(&d[0]),
            &["SC1000", "SC2000", "SC2001", "SC2002", "SC1001"]
        );
    }

    #[test]
    fn disable_range() {
        let d = parse_shellcheck_directive("# shellcheck disable=SC1000-SC1005");
        assert_eq!(d.len(), 1);
        assert_eq!(
            rules(&d[0]),
            &["SC1000", "SC1001", "SC1002", "SC1003", "SC1004", "SC1005"],
        );
    }

    #[test]
    fn disable_all() {
        let d = parse_shellcheck_directive("# shellcheck disable=all");
        assert_eq!(d.len(), 1);
        assert_eq!(rules(&d[0]), &["all"]);
    }

    #[test]
    fn source_directive() {
        let d = parse_shellcheck_directive("# shellcheck source=foo.sh");
        assert_eq!(d.len(), 1);
        assert!(matches!(&d[0], Directive::Source { .. }));
        assert_eq!(path(&d[0]), "foo.sh");
    }

    #[test]
    fn source_directive_strips_trailing_comment() {
        let d = parse_shellcheck_directive("# shellcheck source=/dev/null # a comment");
        assert_eq!(d.len(), 1);
        assert_eq!(path(&d[0]), "/dev/null");
    }

    #[test]
    fn source_path_directive() {
        let d = parse_shellcheck_directive("# shellcheck source-path=src/examples");
        assert_eq!(d.len(), 1);
        assert!(matches!(&d[0], Directive::SourcePath { .. }));
        assert_eq!(path(&d[0]), "src/examples");

        let d2 = parse_shellcheck_directive("# shellcheck source-path=SCRIPTDIR");
        assert_eq!(d2.len(), 1);
        assert_eq!(path(&d2[0]), "SCRIPTDIR");
    }

    #[test]
    fn multiple_known_directives_on_one_line() {
        // enable= and shell= are not Directive variants; only disable= is returned
        let d = parse_shellcheck_directive(
            "# shellcheck cats=dogs disable=SC1234,SC2345 enable=foo shell=bash",
        );
        assert_eq!(d.len(), 1);
        assert_eq!(rules(&d[0]), &["SC1234", "SC2345"]);
    }

    #[test]
    fn no_shellcheck_keyword_returns_empty() {
        assert!(parse_shellcheck_directive("# foo bar").is_empty());
    }

    #[test]
    fn invalid_directives_do_not_panic() {
        assert!(parse_shellcheck_directive("# shellcheck").is_empty());
        assert!(parse_shellcheck_directive("# shellcheck disable = ").is_empty());
        // inverted range → empty rules list
        let d = parse_shellcheck_directive("# shellcheck disable=SC2-SC1");
        assert_eq!(d.len(), 1);
        assert!(rules(&d[0]).is_empty());
        // non-numeric suffix → treated as literal
        let d2 = parse_shellcheck_directive("# shellcheck disable=SC0-SC-1");
        assert_eq!(d2.len(), 1);
        assert_eq!(rules(&d2[0]), &["SC0-SC-1"]);
    }
}
