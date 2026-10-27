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
const DNS_SUFFIXES: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "edu", "gov", "co", "uk", "de", "fr", "ru", "info",
    "biz", "xyz", "invalid",
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

fn private_rooted_location(token: &str) -> bool {
    token.match_indices('/').any(|(index, _)| {
        let boundary = token[..index]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && !matches!(c, '.' | '_' | '-'));
        boundary && starts_with_private_root(&token[index..])
    })
}

fn starts_with_private_root(token: &str) -> bool {
    let Some(path) = token.strip_prefix('/') else {
        return false;
    };
    let mut parts = path.split('/');
    let private_root = match parts.next().unwrap_or("").to_ascii_lowercase().as_str() {
        "home" | "users" | "root" | "mnt" | "media" | "volumes" => true,
        "var" => parts
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case("home")),
        "run" => parts
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case("media")),
        _ => false,
    };
    private_root && parts.any(|part| !part.is_empty())
}

fn home_relative_location(token: &str) -> bool {
    token
        .strip_prefix('~')
        .and_then(|path| path.split_once('/'))
        .is_some_and(|(_, path)| !path.is_empty())
}

fn windows_drive_location(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && bytes.get(2).is_some_and(|c| matches!(c, b'/' | b'\\'))
}

fn windows_location(token: &str) -> bool {
    let unc = token
        .strip_prefix("\\\\\\\\")
        .or_else(|| token.strip_prefix("\\\\"))
        .and_then(|rest| rest.split_once(['\\', '/']))
        .is_some_and(|(host, _)| !host.is_empty());
    windows_drive_location(token) || unc
}

fn authority_host(authority: &str) -> &str {
    authority
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
}

fn scheme_relative_host(token: &str) -> Option<String> {
    let address = token.strip_prefix("//")?;
    let authority = address.split(['/', '?', '#']).next()?;
    let host = authority_host(authority).to_ascii_lowercase();
    let suffix = host.rsplit('.').next()?;
    (host_like(&host) && DNS_SUFFIXES.contains(&suffix)).then_some(host)
}

fn token_violation(token: &str) -> Option<&'static str> {
    let token = token.trim_end_matches(['.', ',', ':', '!', '?']);
    if token.is_empty() {
        return None;
    }
    if let Some((scheme, address)) = token.split_once("://")
        && !scheme.is_empty()
    {
        let authority = address.split(['/', '?', '#']).next().unwrap_or("");
        if !authority.is_empty() {
            let host = authority_host(authority).to_ascii_lowercase();
            return (!HOSTS.contains(&host.as_str())).then_some("unapproved URL host");
        }
        return url_path_violation(address);
    }
    if let Some(path) = token.strip_prefix("file:/")
        && windows_drive_location(path)
    {
        return Some("private filesystem location");
    }
    if let Some(host) = scheme_relative_host(token) {
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
    path_violation(token)
}

fn url_path_violation(path: &str) -> Option<&'static str> {
    let path = path.split(['?', '#']).next().unwrap_or("");
    if path.strip_prefix('/').is_some_and(windows_drive_location) {
        return Some("private filesystem location");
    }
    path_violation(path)
}

fn path_violation(token: &str) -> Option<&'static str> {
    if private_rooted_location(token) || home_relative_location(token) || windows_location(token) {
        return Some("private filesystem location");
    }
    if escapes_tree(token) {
        return Some("path escapes the working tree");
    }
    None
}

fn violation(text: &str) -> Option<&'static str> {
    text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\''
                    | '`'
                    | '<'
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
    let mut in_hunk = false;
    for (number, line) in diff.lines().enumerate() {
        if line.starts_with("diff ") {
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        let Some(text) = line.strip_prefix('+') else {
            continue;
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
    let remote = std::env::args()
        .nth(1)
        .ok_or("missing destination remote")?;
    check_updates(&remote, &input)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE_ROOTS: &[&str] = &[
        "home",
        "Users",
        "root",
        "var/home",
        "mnt",
        "media",
        "run/media",
        "Volumes",
    ];

    fn root_case_variants(root: &str) -> Vec<String> {
        let mut variants = vec![root.to_ascii_lowercase(), root.to_ascii_uppercase()];
        for (index, _) in root.split('/').enumerate() {
            let parts: Vec<_> = root
                .split('/')
                .enumerate()
                .map(|(part_index, part)| {
                    if part_index == index {
                        part.to_ascii_uppercase()
                    } else {
                        part.to_ascii_lowercase()
                    }
                })
                .collect();
            variants.push(parts.join("/"));
        }
        variants
    }

    #[test]
    fn rejects_private_rooted_locations() {
        for root in PRIVATE_ROOTS
            .iter()
            .flat_map(|root| root_case_variants(root))
        {
            for tail in ["person", "person/capture", "person/", "/person"] {
                let path = ["", &root, tail].join("/");
                assert_eq!(
                    violation(&path),
                    Some("private filesystem location"),
                    "missed {path}"
                );
            }
        }
    }

    #[test]
    fn requires_a_segment_after_a_private_root() {
        for root in PRIVATE_ROOTS
            .iter()
            .flat_map(|root| root_case_variants(root))
        {
            for prefix in ["", "!", "**", "file:"] {
                for tail in ["", "/", "//"] {
                    let path = format!("{prefix}{}{tail}", ["", &root].join("/"));
                    assert_eq!(violation(&path), None, "rejected {path}");
                }
            }
            let relative = [root.as_str(), "person"].join("/");
            let other_root = ["", &format!("{root}extra"), "person"].join("/");
            for path in [relative, other_root] {
                assert_eq!(violation(&path), None, "rejected {path}");
            }
        }
    }

    #[test]
    fn checks_private_roots_after_punctuation_only_at_path_boundaries() {
        for root in PRIVATE_ROOTS
            .iter()
            .flat_map(|root| root_case_variants(root))
        {
            let path = ["", &root, "person"].join("/");
            for prefix in ["!", "*", "**", ":", "file:", "note:!", "—"] {
                let value = format!("{prefix}{path}");
                assert_eq!(
                    violation(&value),
                    Some("private filesystem location"),
                    "{value}"
                );
            }
            for prefix in ["a", "Z", "0", ".", "_", "-", "é", "９", "relative"] {
                let value = format!("{prefix}{path}");
                assert_eq!(violation(&value), None, "rejected {value}");
            }
        }
        for prefix in ["!", "*", "**", ":", "file:"] {
            for path in [
                "/target/",
                "/usr/bin/tool",
                "/arbitrary/token",
                "/[a-z]+/gi",
            ] {
                let value = format!("{prefix}{path}");
                assert_eq!(violation(&value), None, "rejected {value}");
            }
        }
    }

    #[test]
    fn checks_file_url_paths_without_requiring_an_authority() {
        for slash_count in [1, 3] {
            let prefix = format!("file:{}", "/".repeat(slash_count));
            for root in PRIVATE_ROOTS
                .iter()
                .flat_map(|root| root_case_variants(root))
            {
                let value = format!("{prefix}{}", [root.as_str(), "person"].join("/"));
                assert_eq!(
                    violation(&value),
                    Some("private filesystem location"),
                    "{value}"
                );
            }
            for path in ["", "usr/bin/tool", "dev/null", "home", "var/home/"] {
                let value = format!("{prefix}{path}");
                assert_eq!(violation(&value), None, "rejected {value}");
            }
        }
        let host = ["service", "invalid"].join(".");
        let value = ["file:", "", &host, "usr/bin/tool"].join("/");
        assert_eq!(violation(&value), Some("unapproved URL host"));
        let value = ["file:", "", "github.com", "usr/bin/tool"].join("/");
        assert_eq!(violation(&value), None);
        let value = ["file:", "", "", "", &host, "path"].join("/");
        assert_eq!(violation(&value), None);
    }

    #[test]
    fn rejects_windows_drive_paths_in_file_urls() {
        for slash_count in [1, 3] {
            for drive in ["C", "z"] {
                for separator in ["/", "\\"] {
                    let path = [format!("{drive}:"), "capture".into()].join(separator);
                    let value = format!("file:{}{path}", "/".repeat(slash_count));
                    assert_eq!(
                        violation(&value),
                        Some("private filesystem location"),
                        "missed {value}"
                    );
                }
            }
        }
    }

    #[test]
    fn limits_slash_prefixed_drive_detection_to_url_paths() {
        let drive = ["C:", "capture"].join("/");
        for prefix in ["/", "note:/", "relative/", "file:////", "https://github.com/"] {
            let value = format!("{prefix}{drive}");
            assert_eq!(violation(&value), None, "rejected {value}");
        }
        let value = format!("https:///{drive}");
        assert_eq!(violation(&value), Some("private filesystem location"));
    }

    #[test]
    fn checks_only_scheme_relative_hosts_with_listed_dns_suffixes() {
        for suffix in DNS_SUFFIXES {
            let host = ["service", suffix].join(".");
            for host in [host.clone(), host.to_ascii_uppercase()] {
                for tail in ["", "/path", ":443/path", "?query", "#fragment"] {
                    let value = ["", "", &format!("{host}{tail}")].join("/");
                    assert_eq!(violation(&value), Some("unapproved URL host"), "{value}");
                }
            }
        }
        for host in HOSTS {
            for tail in ["", "/path", ":443/path", "?query", "#fragment"] {
                let value = ["", "", &format!("{}{tail}", host.to_ascii_uppercase())].join("/");
                assert_eq!(violation(&value), None, "rejected {value}");
            }
        }
        for authority in [
            "",
            "comment",
            "comment/path",
            "state.rs",
            "a..com",
            "-",
            "text+value.com",
        ] {
            let value = ["", "", authority].join("/");
            assert_eq!(violation(&value), None, "rejected {value}");
        }
        let host = ["service", "invalid"].join(".");
        for prefix in ["", "/", "///", "!//", "word//"] {
            let value = format!("{prefix}{host}/path");
            assert_eq!(violation(&value), None, "rejected {value}");
        }
        let authority = format!("user:token@{host}:443");
        let value = ["", "", &authority, "path"].join("/");
        assert_eq!(violation(&value), Some("unapproved URL host"));
    }

    #[test]
    fn rejects_home_relative_locations_with_or_without_a_user() {
        for home in ["~", "~person"] {
            for tail in ["capture", "data/capture", "/"] {
                let path = [home, tail].join("/");
                assert_eq!(
                    violation(&path),
                    Some("private filesystem location"),
                    "missed {path}"
                );
            }
            for path in [home.to_owned(), [home, ""].join("/")] {
                assert_eq!(violation(&path), None, "rejected {path}");
            }
        }
    }

    #[test]
    fn rejects_tree_escapes() {
        for parts in [
            vec!["..", "capture"],
            vec!["cache", "..", "..", "capture"],
            vec![".", "..", "capture"],
        ] {
            for separator in ["/", "\\"] {
                let path = parts.join(separator);
                assert_eq!(
                    violation(&path),
                    Some("path escapes the working tree"),
                    "missed {path}"
                );
            }
        }
        let inside = ["cache", "..", "capture"].join("/");
        assert_eq!(violation(&inside), None);
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
            assert_eq!(
                violation(&value),
                Some("unapproved URL host"),
                "missed {value}"
            );
        }
    }

    #[test]
    fn permits_markup_closing_tags_and_complete_svg() {
        for value in [
            "</div>",
            "<div>\ncontent\n</div>",
            "<svg></svg>",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g fill="currentColor"></g></svg>"#,
            include_str!(concat!(".", ".", "/", "assets/icons/wireroom-mono.svg")),
        ] {
            assert_eq!(violation(value), None, "rejected {value}");
            for line in value.lines() {
                assert_eq!(violation(line), None, "rejected line {line}");
            }
        }
    }

    #[test]
    fn permits_regular_expressions_and_other_slash_tokens() {
        for value in [
            r"const words = /[a-z]+/gi;",
            r"const opening = /</g;",
            "/",
            "//",
            "///",
            "/*",
            "/**",
            "/dev/null",
            "/usr/bin/tool",
            "/private/capture",
            "/var/log/messages",
            "/run/service/socket",
            "/arbitrary/token",
        ] {
            assert_eq!(violation(value), None, "rejected {value}");
        }
    }

    #[test]
    fn rejects_paths_inside_and_outside_angle_brackets() {
        let path = ["", "home", "person"].join("/");
        for value in [path.clone(), format!("<{path}>"), format!("<g></g>{path}")] {
            assert_eq!(
                violation(&value),
                Some("private filesystem location"),
                "missed {value}"
            );
        }
    }

    #[test]
    fn permits_bare_hosts_and_property_access() {
        for value in [
            "service.invalid",
            "SERVICE.INVALID",
            "service.invalid/path",
            "service.invalid:443",
            "state.app",
            "result.org",
            "console.info(value)",
            "GitHub.com",
        ] {
            assert_eq!(violation(value), None, "rejected {value}");
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
            assert_eq!(
                violation(&address),
                Some("unapproved URL host"),
                "missed {address}"
            );
        }
        for host in [host.as_str(), "service"] {
            let address = format!("git@{host}:team/repo");
            assert_eq!(
                violation(&address),
                Some("unapproved host"),
                "missed {address}"
            );
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
    fn distinguishes_backslash_text_from_windows_locations() {
        for value in [r"\", r"\n", r"\\n", r"\foo", r"\\host", r"C:foo"] {
            assert_eq!(violation(value), None, "rejected {value}");
        }
        for value in [
            ["", "", "host", "share"].join("\\"),
            ["", "", "host", ""].join("\\"),
            [concat!("\\", "\\", "host"), "share"].join("/"),
            ["C:", "foo"].join("\\"),
            ["C:", "foo"].join("/"),
            ["z:", ""].join("/"),
            ["z:", ""].join("\\"),
        ] {
            assert_eq!(
                violation(&value),
                Some("private filesystem location"),
                "missed {value}"
            );
        }
    }

    #[test]
    fn checks_direct_and_doubled_unc_prefixes_with_a_host_and_separator() {
        for width in [1, 2] {
            let separator = "\\".repeat(width);
            for tail in ["", "share", "share/path"] {
                let path = ["", "", "host", tail].join(&separator);
                for value in [path.clone(), format!("let path = \"{path}\";")] {
                    assert_eq!(
                        violation(&value),
                        Some("private filesystem location"),
                        "{value}"
                    );
                }
                let path = format!("{}host/{tail}", separator.repeat(2));
                assert_eq!(
                    violation(&path),
                    Some("private filesystem location"),
                    "{path}"
                );
            }
            for tail in ["", "host"] {
                let path = format!("{}{tail}", separator.repeat(2));
                assert_eq!(violation(&path), None, "rejected {path}");
            }
        }
        for count in 1..=8 {
            let path = "\\".repeat(count);
            assert_eq!(violation(&path), None, "rejected {path}");
        }
    }

    #[test]
    fn reports_one_based_diff_line_numbers() {
        let added = format!("+{}", ["", "home", "person"].join("/"));
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
            Err("commit test: private filesystem location in added text at diff line 7".into())
        );
    }

    fn added_text_diff(file: &str, text: &str) -> String {
        let header = ["b", file].join("/");
        format!("diff --git a/{file} {header}\n+++ {header}\n@@ -0,0 +1 @@\n+{text}")
    }

    #[test]
    fn checks_added_lines_starting_with_two_pluses() {
        let path = ["", "home", "person"].join("/");
        for prefix in ["++ ", "++prefix "] {
            let diff = added_text_diff("notes.txt", &format!("{prefix}{path}"));
            assert_eq!(
                check_added_text("test", &diff),
                Err("commit test: private filesystem location in added text at diff line 4".into()),
                "{diff}"
            );
        }
    }

    #[test]
    fn added_header_text_does_not_end_the_hunk() {
        let path = ["", "home", "person"].join("/");
        let diff = format!(
            "diff --git a/notes.txt b/notes.txt\n+++ b/notes.txt\n@@ -0,0 +1,2 @@\n+++ b/.gitignore\n+{path}"
        );
        assert_eq!(
            check_added_text("test", &diff),
            Err("commit test: private filesystem location in added text at diff line 5".into())
        );
    }

    #[test]
    fn permits_anchored_patterns_in_every_file() {
        for file in [
            ".gitignore",
            "nested/.gitignore",
            "notes.txt",
            ".dockerignore",
        ] {
            for pattern in ["/target/", "!/target/", "# /target/", " /target/"] {
                let diff = added_text_diff(file, pattern);
                assert_eq!(check_added_text("test", &diff), Ok(()), "{diff}");
            }
        }
    }

    #[test]
    fn skips_headers_until_each_hunk() {
        let path = ["", "home", "person"].join("/");
        let header = format!("+++ {path}");
        let first = [
            "diff --git a/notes.txt b/notes.txt",
            &header,
            "@@ -1 +1 @@",
            "-old text",
            "+safe text",
        ]
        .join("\n");
        let diff = format!("{first}\ndiff --git a/next.txt b/next.txt\n{header}");
        assert_eq!(check_added_text("test", &diff), Ok(()));
        let with_hunk = format!("{diff}\n@@ -0,0 +1 @@\n+{path}");
        assert_eq!(
            check_added_text("test", &with_hunk),
            Err("commit test: private filesystem location in added text at diff line 9".into())
        );
    }

    #[test]
    fn handles_deleted_files_without_skipping_later_additions() {
        let path = ["", "home", "person"].join("/");
        let diff = format!(
            "diff --git a/notes.txt b/notes.txt\n--- a/notes.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-{path}"
        );
        assert_eq!(check_added_text("test", &diff), Ok(()));
        let next_file = format!("{diff}\n{}", added_text_diff("next.txt", &path));
        assert_eq!(
            check_added_text("test", &next_file),
            Err("commit test: private filesystem location in added text at diff line 9".into())
        );
    }

    #[test]
    fn checks_private_locations_in_gitignore_without_an_exemption() {
        let path = ["", "home", "person"].join("/");
        for prefix in ["", "# ", " ", "/target/ "] {
            let diff = added_text_diff(".gitignore", &format!("{prefix}{path}"));
            assert_eq!(
                check_added_text("test", &diff),
                Err("commit test: private filesystem location in added text at diff line 4".into())
            );
        }
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
