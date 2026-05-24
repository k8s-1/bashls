pub const LIST: &[&str] = &[
    "!", "[[", "]]", "{", "}", "case", "do", "done", "elif", "else", "esac", "fi", "for",
    "function", "if", "in", "select", "then", "time", "until", "while",
];

static RESERVED_SET: std::sync::LazyLock<std::collections::HashSet<&'static str>> =
    std::sync::LazyLock::new(|| LIST.iter().copied().collect());

pub fn is_reserved_word(word: &str) -> bool {
    RESERVED_SET.contains(word)
}
