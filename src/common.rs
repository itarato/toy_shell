pub(crate) const SHELL_BUILTIN_COMMANDS: [&'static str; 6] =
    ["echo", "type", "exit", "pwd", "cd", "history"];

pub(crate) fn has_space(s: &str) -> bool {
    let chars = s.chars().collect::<Vec<_>>();
    chars.len() >= 2 && !chars[0].is_whitespace() && chars.contains(&' ')
}

pub(crate) fn shared_prefix_len(lhs: &str, rhs: &str) -> usize {
    let len = lhs.len().min(rhs.len());
    for i in 0..len {
        if lhs[i..=i] != rhs[i..=i] {
            return i;
        }
    }

    len
}

pub(crate) fn split_last_cmd_line_arg(s: &str) -> Option<(&str, &str)> {
    s.split(' ')
        .last()
        .map(|part| (&s[..s.len() - part.len()], part))
}

pub(crate) fn split_path_match_to_dir_and_prefix(s: &str) -> (Option<String>, String) {
    let mut parts = s.split('/').collect::<Vec<_>>();

    if parts.is_empty() {
        panic!();
    } else if parts.len() == 1 {
        (None, s.to_string())
    } else {
        let last = parts.pop().unwrap();
        let mut dir = parts.join("/").to_string();
        dir.push('/');
        (Some(dir), last.to_string())
    }
}

pub(crate) fn matching_files(prefix: &str, dir: &Option<String>) -> Vec<String> {
    std::fs::read_dir(dir.as_ref().unwrap_or(&String::from(".")))
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
