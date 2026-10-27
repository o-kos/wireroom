//! Generic hygiene checks for outgoing commit messages, filenames and added text.

use std::{
    io::{self, Read},
    process::Command,
};

const HOSTS: &[&str] = &[
    "github.com",
    "crates.io",
    "index.crates.io",
    "doc.rust-lang.org",
    "keepachangelog.com",
    "www.w3.org",
];
// Bare names use common DNS suffixes to distinguish hosts from source expressions.
// URLs are checked for every scheme and suffix, independently of this heuristic.
const DNS_SUFFIXES: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "edu", "gov", "co", "uk", "de", "fr", "ru", "info",
    "biz", "xyz", "invalid",
];
const EXTENSIONS: &[&str] = &[
    "rs", "md", "toml", "lock", "yml", "yaml", "json", "h", "c", "cpp", "txt", "sh", "exe", "dll",
    "so", "dylib", "a", "lib",
];

fn escapes_tree(value: &str) -> bool {
    let mut depth = 0;
    for part in value.split(['/', '\\']) {
        match part {
            ".." if depth == 0 => return true,
            ".." => depth -= 1,
            "" | "." => (),
            _ => depth += 1,
        }
    }
    false
}

fn host_like(value: &str) -> bool {
    let labels: Vec<_> = value.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && labels.last().is_some_and(|suffix| {
            suffix.len() >= 2 && suffix.chars().all(|c| c.is_ascii_alphabetic())
        })
}

fn token_violation(token: &str) -> Option<&'static str> {
    let token = token.trim_end_matches(['.', ',', ':', '!', '?']);
    if token.is_empty() {
        return None;
    }
    if let Some((scheme, address)) = token.split_once("://")
        && !scheme.is_empty()
        && !address.is_empty()
    {
        let authority = address.split(['/', '?', '#']).next().unwrap_or("");
        let host_port = authority.rsplit('@').next().unwrap_or("");
        let host = host_port.split(':').next().unwrap_or("").to_ascii_lowercase();
        return (!HOSTS.contains(&host.as_str())).then_some("unapproved URL host");
    }
    if let Some((user, address)) = token.split_once('@') {
        if let Some((host, path)) = address.split_once(':')
            && !user.is_empty()
            && !host.is_empty()
            && !path.is_empty()
        {
            return (!HOSTS.contains(&host.to_ascii_lowercase().as_str()))
                .then_some("unapproved host");
        }
        if !user.is_empty() && host_like(address) {
            return Some("e-mail address");
        }
    }
    let drive = token.as_bytes()[0].is_ascii_alphabetic()
        && token.as_bytes().get(1) == Some(&b':')
        && token
            .as_bytes()
            .get(2)
            .is_some_and(|c| matches!(c, b'/' | b'\\'));
    let unc = token
        .strip_prefix("\\\\")
        .and_then(|rest| rest.split_once(['\\', '/']))
        .is_some_and(|(host, _)| !host.is_empty());
    if drive
        || token.starts_with('/')
        || unc
        || token.starts_with('~') && token.as_bytes().get(1) == Some(&b'/')
    {
        // Standalone comment delimiters are syntax, and the null device is the same on
        // every Unix host, so neither discloses anything about an environment.
        if !matches!(token, "/" | "//" | "///" | "/*" | "/**" | "/dev/null") {
            return Some("absolute filesystem path");
        }
    }
    if escapes_tree(token) {
        return Some("path escapes the working tree");
    }
    let host = token.split(['/', ':']).next().unwrap_or("").to_ascii_lowercase();
    if host_like(&host)
        && host
            .rsplit('.')
            .next()
            .is_some_and(|suffix| DNS_SUFFIXES.contains(&suffix))
        && !HOSTS.contains(&host.as_str())
        && !host
            .rsplit('.')
            .next()
            .is_some_and(|ext| EXTENSIONS.contains(&ext))
        && !host.contains('_')
    {
        return Some("unapproved host");
    }
    None
}

fn strip_markup_closing_tag<'a>(text: &'a str, earlier: &str) -> &'a str {
    let Some((tag, rest)) = text.split_once('>') else {
        return text;
    };
    let Some(name) = tag.strip_prefix('/') else {
        return text;
    };
    if name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b':'))
        && earlier.split('<').skip(1).any(|part| {
            part.strip_prefix(name).is_some_and(|rest| {
                rest.starts_with(|c: char| c.is_whitespace() || matches!(c, '>' | '/'))
            })
        })
    {
        return rest;
    }
    text
}

fn violation(text: &str) -> Option<&'static str> {
    let mut offset = 0;
    text.split('<')
        .enumerate()
        .flat_map(|(index, part)| {
            let earlier = &text[..offset];
            offset += part.len() + 1;
            let part = if index == 0 {
                part
            } else {
                strip_markup_closing_tag(part, earlier)
            };
            part.split(|c: char| {
                c.is_whitespace()
                    || matches!(
                        c,
                        '"' | '\''
                            | '`'
                            | '>'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '{'
                            | '}'
                            | '='
                            | ';'
                            | ','
                            | '|'
                    )
            })
        })
        .find_map(token_violation)
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", args[0]));
    }
    String::from_utf8(output.stdout).map_err(|_| "non-UTF-8 content requires manual review".into())
}

fn check_commit(commit: &str) -> Result<(), String> {
    let message = git(&["show", "-s", "--format=%B", commit])?;
    if let Some(reason) = violation(&message) {
        return Err(format!("commit {commit}: {reason} in message"));
    }
    let names = git(&[
        "diff-tree",
        "--root",
        "--diff-merges=first-parent",
        "--no-commit-id",
        "--name-only",
        "-r",
        "-z",
        commit,
    ])?;
    for name in names.split('\0').filter(|name| !name.is_empty()) {
        if let Some(reason) = violation(name) {
            return Err(format!("commit {commit}: {reason} in filename"));
        }
    }
    let diff = git(&[
        "show",
        "--format=",
        "--root",
        "--diff-merges=first-parent",
        "--no-ext-diff",
        "--no-textconv",
        "--unified=0",
        commit,
    ])?;
    check_added_text(commit, &diff)
}

fn check_added_text(commit: &str, diff: &str) -> Result<(), String> {
    let mut current_file = None;
    let mut in_hunk = false;
    for (number, line) in diff.lines().enumerate() {
        if line.starts_with("diff ") {
            current_file = None;
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            if let Some(header) = line.strip_prefix("+++ ") {
                let path = header
                    .strip_prefix('"')
                    .and_then(|path| path.strip_suffix('"'))
                    .unwrap_or(header);
                current_file = (path != "/dev/null").then(|| path.strip_prefix("b/").unwrap_or(path));
            }
            continue;
        }
        let Some(text) = line.strip_prefix('+') else {
            continue;
        };
        let text = if current_file.is_some_and(|path| path.rsplit('/').next() == Some(".gitignore")) {
            text.strip_prefix('/')
                .or_else(|| text.strip_prefix("!/"))
                .unwrap_or(text)
        } else {
            text
        };
        if let Some(reason) = violation(text) {
            return Err(format!(
                "commit {commit}: {reason} in added text at diff line {}",
                number + 1
            ));
        }
    }
    Ok(())
}

fn new_branch_history(remote: &str, commit: &str) -> Result<String, String> {
    let refs = git(&[
        "for-each-ref",
        "--format=%(objectname)",
        &format!("refs/remotes/{remote}/"),
    ])?;
    let mut args = vec!["rev-list", commit, "--not"];
    args.extend(refs.lines());
    git(&args)
}

fn check_updates(remote: &str, input: &str) -> Result<(), String> {
    let mut commits = std::collections::BTreeSet::new();
    for line in input.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 4 {
            return Err("invalid pre-push ref update".into());
        }
        if fields[2] == "refs/heads/main" {
            return Err("direct pushes to main are refused; use a Pull Request".into());
        }
        if fields[1].chars().all(|c| c == '0') {
            continue;
        }
        let history = if fields[3].chars().all(|c| c == '0') {
            new_branch_history(remote, fields[1])?
        } else {
            git(&["rev-list", &format!("{}..{}", fields[3], fields[1])])?
        };
        commits.extend(history.lines().map(str::to_owned));
    }
    for commit in commits {
        check_commit(&commit)?;
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    let remote = std::env::args().nth(1).ok_or("missing destination remote")?;
    check_updates(&remote, &input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_generic_private_content() {
        for value in [
            ["", "private", "capture"].join("/"),
            ["C:", "private", "capture"].join("\\"),
            ["..", "capture"].join("/"),
            ["person".to_owned(), ["mail", "invalid"].join(".")].join("@"),
            [
                "https:".to_owned(),
                String::new(),
                ["service", "invalid"].join("."),
                "data".into(),
            ]
            .join("/"),
            ["service", "invalid"].join("."),
        ] {
            assert!(violation(&value).is_some(), "missed {value}");
        }
    }

    #[test]
    fn permits_repository_paths_and_approved_urls() {
        for value in [
            "./foo",
            "crates/cli/src/main.rs",
            "Cargo.toml",
            "// Comment",
            "cmd < /dev/null",
            "https://github.com/actions/checkout",
        ] {
            assert_eq!(violation(value), None, "rejected {value}");
        }
    }

    #[test]
    fn permits_svg_namespace_but_rejects_unapproved_hosts() {
        for value in [
            "http://www.w3.org/2000/svg",
            r#"xmlns="http://www.w3.org/2000/svg""#,
        ] {
            assert_eq!(violation(value), None, "rejected {value}");
        }
        let host = ["service", "invalid"].join(".");
        let url = ["http:", "", &host, "2000/svg"].join("/");
        for value in [url.clone(), format!(r#"xmlns="{url}""#)] {
            assert_eq!(violation(&value), Some("unapproved URL host"), "missed {value}");
        }
    }

    #[test]
    fn permits_markup_closing_tags_and_complete_svg() {
        for value in [
            "<svg></svg>",
            "<g></g>",
            "<A0-_.:z></A0-_.:z>",
            "<g/></g>",
            concat!("<g", "\t>", "<", "/", "g>"),
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g fill="currentColor"></g></svg>"#,
            include_str!(concat!(".", ".", "/", "assets/icons/wireroom-mono.svg")),
        ] {
            assert_eq!(violation(value), None, "rejected {value}");
        }
    }

    #[test]
    fn rejects_closing_tags_without_an_earlier_matching_opening_tag() {
        for name in ["private", "svg", "g", "A0-_.:z"] {
            let closing = ["<", "/", name, ">"].concat();
            for value in [
                closing.clone(),
                format!("<unrelated>{closing}"),
                format!("{closing}<{name}>"),
                format!("<{name}extra>{closing}"),
                format!("<{}>{closing}", name.to_uppercase()),
            ] {
                assert_eq!(
                    violation(&value),
                    Some("absolute filesystem path"),
                    "missed {value}"
                );
            }
        }
        let closing = ["<", "/", "private", ">"].concat();
        assert_eq!(
            violation(&format!("<g>{closing}")),
            Some("absolute filesystem path")
        );
    }

    #[test]
    fn rejects_paths_inside_and_outside_angle_brackets() {
        let path = ["", "private", "capture"].join("/");
        for value in [path.clone(), format!("<{path}>"), format!("<g></g>{path}")] {
            assert_eq!(
                violation(&value),
                Some("absolute filesystem path"),
                "missed {value}"
            );
        }
    }

    #[test]
    fn rejects_slash_tokens_that_are_not_well_formed_closing_tags() {
        for name in [
            "svg", "g", "1svg", "_svg", "é", "svg ", "svg attr", "svg/", "svg!",
        ] {
            let token = ["", name].join("/");
            for value in [token.clone(), format!("<{token}")] {
                assert_eq!(
                    violation(&value),
                    Some("absolute filesystem path"),
                    "missed {value}"
                );
            }
            let value = format!("<{token}>");
            assert_eq!(
                violation(&value),
                Some("absolute filesystem path"),
                "missed {value}"
            );
        }
    }

    #[test]
    fn checks_bare_hosts_case_insensitively() {
        let host = ["SERVICE", "INVALID"].join(".");
        assert_eq!(violation(&host), Some("unapproved host"));
        for host in ["GitHub.com", "GITHUB.COM"] {
            assert_eq!(violation(host), None, "rejected {host}");
        }
    }

    #[test]
    fn checks_url_authorities_and_scp_hosts() {
        let host = ["service", "invalid"].join(".");
        for address in [
            ["https:", "", &format!("github.com:token@{host}"), "repo"].join("/"),
            ["https:", "", &format!("user@github.com@{host}:443"), "repo"].join("/"),
            ["https:", "", &format!("{host}?query")].join("/"),
            ["https:", "", &format!("{host}#fragment")].join("/"),
        ] {
            assert_eq!(violation(&address), Some("unapproved URL host"), "missed {address}");
        }
        for host in [host.as_str(), "service"] {
            let address = format!("git@{host}:team/repo");
            assert_eq!(violation(&address), Some("unapproved host"), "missed {address}");
        }
        let email = ["person", &host].join("@");
        assert_eq!(violation(&email), Some("e-mail address"));
        for address in [
            "https://GitHub.COM:443/owner/repo",
            "https://user:token@github.com/owner/repo",
            "https://user@github.com?query",
            "https://github.com#fragment",
            "git@github.com:owner/repo",
            "git@GitHub.COM:owner/repo",
        ] {
            assert_eq!(violation(address), None, "rejected {address}");
        }
    }

    #[test]
    fn policy_source_obeys_its_own_rules() {
        for (number, line) in include_str!("content_policy.rs").lines().enumerate() {
            assert_eq!(violation(line), None, "line {}: {line}", number + 1);
        }
    }

    #[test]
    fn distinguishes_backslash_text_from_absolute_windows_paths() {
        for value in [r"\", r"\n", r"\\n", r"\foo", r"\\host", r"C:foo"] {
            assert_eq!(violation(value), None, "rejected {value}");
        }
        for value in [
            ["", "", "host", "share"].join("\\"),
            ["C:", "foo"].join("\\"),
            ["C:", "foo"].join("/"),
        ] {
            assert_eq!(
                violation(&value),
                Some("absolute filesystem path"),
                "missed {value}"
            );
        }
    }

    #[test]
    fn reports_one_based_diff_line_numbers() {
        let added = format!("+{}", ["", "private", "capture"].join("/"));
        let diff = [
            "diff --git a/file b/file",
            "--- a/file",
            "+++ b/file",
            "@@ -1 +1,2 @@",
            "-old text",
            "+safe text",
            &added,
        ]
        .join("\n");
        assert_eq!(
            check_added_text("test", &diff),
            Err("commit test: absolute filesystem path in added text at diff line 7".into())
        );
    }

    fn added_text_diff(file: &str, text: &str) -> String {
        let header = ["b", file].join("/");
        format!("diff --git a/{file} {header}\n+++ {header}\n@@ -0,0 +1 @@\n+{text}")
    }

    #[test]
    fn checks_added_lines_starting_with_two_pluses() {
        let path = ["", "private", "capture"].join("/");
        for prefix in ["++ ", "++prefix "] {
            let diff = added_text_diff("notes.txt", &format!("{prefix}{path}"));
            assert_eq!(
                check_added_text("test", &diff),
                Err("commit test: absolute filesystem path in added text at diff line 4".into()),
                "{diff}"
            );
        }
    }

    #[test]
    fn added_header_text_does_not_change_current_file() {
        let pattern = ["", "target", ""].join("/");
        let header_text = ["b", ".gitignore"].join("/");
        let diff = format!(
            "diff --git a/notes.txt b/notes.txt\n+++ b/notes.txt\n@@ -0,0 +1,2 @@\n+++ {header_text}\n+{pattern}"
        );
        assert_eq!(
            check_added_text("test", &diff),
            Err("commit test: absolute filesystem path in added text at diff line 5".into())
        );
    }

    #[test]
    fn permits_anchored_gitignore_patterns() {
        for file in [".gitignore".to_owned(), ["nested", ".gitignore"].join("/")] {
            for prefix in ["", "!"] {
                let pattern = [prefix, "target", ""].join("/");
                let diff = added_text_diff(&file, &pattern);
                assert_eq!(check_added_text("test", &diff), Ok(()), "{diff}");
            }
        }
    }

    #[test]
    fn handles_quoted_gitignore_headers() {
        for file in [
            ".gitignore".to_owned(),
            ["nested directory", ".gitignore"].join("/"),
            [r"nested\tname", ".gitignore"].join("/"),
        ] {
            let header = ["b", &file].join("/");
            for prefix in ["", "!"] {
                let pattern = [prefix, "target", ""].join("/");
                let diff = format!(
                    "diff --git \"a/{file}\" \"{header}\"\n+++ \"{header}\"\n@@ -0,0 +1 @@\n+{pattern}"
                );
                assert_eq!(check_added_text("test", &diff), Ok(()), "{diff}");
            }
        }
    }

    #[test]
    fn rejects_anchored_patterns_outside_gitignore() {
        let pattern = ["", "target", ""].join("/");
        for file in [
            "notes.txt".to_owned(),
            ".dockerignore".to_owned(),
            ".gitignore.backup".to_owned(),
            [".gitignore", "notes.txt"].join("/"),
        ] {
            let diff = added_text_diff(&file, &pattern);
            assert_eq!(
                check_added_text("test", &diff),
                Err("commit test: absolute filesystem path in added text at diff line 4".into()),
                "{diff}"
            );
        }
    }

    #[test]
    fn checks_gitignore_comments_and_unanchored_lines_unchanged() {
        let pattern = ["", "target", ""].join("/");
        for prefix in ["# ", " # ", "stray ", " "] {
            let diff = added_text_diff(".gitignore", &format!("{prefix}{pattern}"));
            assert_eq!(
                check_added_text("test", &diff),
                Err("commit test: absolute filesystem path in added text at diff line 4".into()),
                "{diff}"
            );
        }
    }

    #[test]
    fn keeps_other_checks_for_gitignore_patterns() {
        let host = ["service", "invalid"].join(".");
        let email = ["person", &host].join("@");
        let url = ["https:", "", &host, "data"].join("/");
        for (text, reason) in [
            (email, "e-mail address"),
            (host, "unapproved host"),
            (url, "unapproved URL host"),
            (["..", "capture"].join("/"), "path escapes the working tree"),
            (
                ["cache", "..", "..", "capture"].join("/"),
                "path escapes the working tree",
            ),
        ] {
            let anchored = ["", &text].join("/");
            let negated = ["!", &anchored].concat();
            for pattern in [&text, &anchored, &negated] {
                let diff = added_text_diff(".gitignore", pattern);
                assert_eq!(
                    check_added_text("test", &diff),
                    Err(format!("commit test: {reason} in added text at diff line 4")),
                    "{diff}"
                );
            }
        }
    }

    #[test]
    fn checks_paths_after_the_gitignore_anchor() {
        let path = ["", "private", "capture"].join("/");
        for prefix in ["", "!"] {
            for pattern in [
                [prefix, &path].join("/"),
                format!("{} {path}", [prefix, "target"].join("/")),
            ] {
                let diff = added_text_diff(".gitignore", &pattern);
                assert_eq!(
                    check_added_text("test", &diff),
                    Err("commit test: absolute filesystem path in added text at diff line 4".into()),
                    "{diff}"
                );
            }
        }
    }

    #[test]
    fn updates_current_file_and_preserves_diff_line_numbers() {
        let pattern = ["", "target", ""].join("/");
        let diff = [
            added_text_diff(".gitignore", &pattern),
            added_text_diff("notes.txt", &pattern),
        ]
        .join("\n");
        assert_eq!(
            check_added_text("test", &diff),
            Err("commit test: absolute filesystem path in added text at diff line 8".into())
        );
    }

    #[test]
    fn handles_deleted_files_and_clears_current_file() {
        let pattern = ["", "target", ""].join("/");
        let null = ["", "dev", "null"].join("/");
        let old = ["a", ".gitignore"].join("/");
        let new = ["b", ".gitignore"].join("/");
        let diff = format!(
            "{}\ndiff --git {old} {new}\n--- {old}\n+++ {null}\n@@ -1 +0,0 @@\n-{pattern}",
            added_text_diff(".gitignore", &pattern)
        );
        assert_eq!(check_added_text("test", &diff), Ok(()));
        let without_file = format!("{diff}\n+{pattern}");
        assert_eq!(
            check_added_text("test", &without_file),
            Err("commit test: absolute filesystem path in added text at diff line 10".into())
        );
        let next_file = format!("{diff}\n{}", added_text_diff(".gitignore", &pattern));
        assert_eq!(check_added_text("test", &next_file), Ok(()));
    }

    #[test]
    fn refuses_main_including_deletion() {
        assert!(
            check_updates("origin", "refs/heads/topic 0000 refs/heads/main 1234")
                .unwrap_err()
                .contains("Pull Request")
        );
    }
}
