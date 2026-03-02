pub(crate) const SHELL_BUILTIN_COMMANDS: [&'static str; 6] =
    ["echo", "type", "exit", "pwd", "cd", "history"];

pub(crate) fn has_space(s: &str) -> bool {
    let chars = s.chars().collect::<Vec<_>>();
    chars.len() >= 2 && !chars[0].is_whitespace() && chars.contains(&' ')
}

pub(crate) fn last_cmd_line_arg(s: &str) -> Option<&str> {
    s.split(' ').last()
}

pub(crate) fn matching_files(prefix: &str, dir: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .unwrap()
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                e.file_name().into_string().ok().and_then(|name| {
                    if name.starts_with(prefix) {
                        Some(name)
                    } else {
                        None
                    }
                })
            })
        })
        .collect()
}
