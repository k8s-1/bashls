pub const LIST: &[&str] = &[
    ".", ":", "[", "alias", "bg", "bind", "break", "builtin", "caller", "cd", "command", "compgen",
    "compopt", "complete", "continue", "declare", "dirs", "disown", "echo", "enable", "eval",
    "exec", "exit", "export", "false", "fc", "fg", "getopts", "hash", "help", "history", "jobs",
    "kill", "let", "local", "logout", "popd", "printf", "pushd", "pwd", "read", "readonly",
    "return", "set", "shift", "shopt", "source", "suspend", "test", "times", "trap", "true",
    "type", "typeset", "ulimit", "umask", "unalias", "unset", "wait",
];

static BUILTIN_SET: std::sync::LazyLock<std::collections::HashSet<&'static str>> =
    std::sync::LazyLock::new(|| LIST.iter().copied().collect());

pub fn is_builtin(word: &str) -> bool {
    BUILTIN_SET.contains(word)
}
