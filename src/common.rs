pub(crate) const SHELL_BUILTIN_COMMANDS: [&'static str; 6] =
    ["echo", "type", "exit", "pwd", "cd", "history"];

pub(crate) fn has_space(s: &str) -> bool {
    let chars = s.chars().collect::<Vec<_>>();
    chars.len() >= 2 && !chars[0].is_whitespace() && chars.contains(&' ')
}

pub(crate) fn common_prefix(subject: &str, current_shared: &Option<String>) -> String {
    if let Some(current) = current_shared.as_ref() {
        let mut len = subject.len().min(current.len());
        for i in 0..len {
            if subject[i..=i] != current[i..=i] {
                len = i;
                break;
            }
        }

        subject[..len].to_string()
    } else {
        subject.to_string()
    }
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
        let dir = parts.join("/").to_string();
        (Some(dir), last.to_string())
    }
}

pub(crate) fn matching_files(
    prefix: &str,
    dir: &Option<String>,
    is_recursive: bool,
) -> Vec<String> {
    // dbg!(prefix, dir);
    let matches: Vec<String> = std::fs::read_dir(dir.as_ref().unwrap_or(&String::from(".")))
        .ok()
        .unwrap()
        .filter_map(|entry_result| {
            entry_result.ok().and_then(|entry| {
                let is_dir = entry
                    .metadata()
                    .map(|metadata| metadata.is_dir())
                    .unwrap_or(false);

                entry.file_name().into_string().ok().and_then(|mut name| {
                    if is_dir {
                        name.push('/');
                    }

                    if name.starts_with(prefix) {
                        Some(name)
                    } else {
                        None
                    }
                })
            })
        })
        .collect();

    if is_recursive && matches.len() == 1 && matches[0].ends_with("/") {
        let subdir = format!(
            "{}{}",
            dir.as_ref()
                .map(|d| format!("{}/", d))
                .unwrap_or("".to_string()),
            &matches[0][..matches[0].len() - 1]
        );
        let sub_matches = matching_files("", &Some(subdir.clone()), false);
        if sub_matches.is_empty() {
            return matches;
        }

        sub_matches
            .into_iter()
            .map(|suffix| format!("{}/{}", &matches[0][..matches[0].len() - 1], suffix))
            .collect()
    } else {
        matches
    }
}

#[cfg(test)]
mod test {
    use crate::common::split_path_match_to_dir_and_prefix;

    #[test]
    fn test_split_path_match_to_dir_and_prefix() {
        assert_eq!(
            (Some("target".to_string()), "d".to_string()),
            split_path_match_to_dir_and_prefix("target/d"),
        );

        assert_eq!(
            (Some("target".to_string()), "".to_string()),
            split_path_match_to_dir_and_prefix("target/"),
        );

        assert_eq!(
            (None, "target".to_string()),
            split_path_match_to_dir_and_prefix("target"),
        );

        assert_eq!(
            (Some("target/debug".to_string()), "build".to_string()),
            split_path_match_to_dir_and_prefix("target/debug/build"),
        );
    }
}
