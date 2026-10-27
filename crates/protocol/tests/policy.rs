use std::{
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_owned()
}

fn validate_sources(metadata: &Value) -> Result<(), String> {
    let root = Path::new(
        metadata["workspace_root"]
            .as_str()
            .ok_or("missing workspace root")?,
    )
    .canonicalize()
    .map_err(|e| e.to_string())?;
    for package in metadata["packages"].as_array().ok_or("missing packages")? {
        let name = package["name"].as_str().ok_or("missing package name")?;
        match package["source"].as_str() {
            Some(source) if source.starts_with("registry+") => (),
            Some(_) => {
                return Err(format!(
                    "{name}: only registry or workspace path sources are permitted"
                ));
            }
            None => {
                let manifest = Path::new(
                    package["manifest_path"]
                        .as_str()
                        .ok_or("missing manifest path")?,
                )
                .canonicalize()
                .map_err(|e| e.to_string())?;
                if !manifest.starts_with(&root) {
                    return Err(format!("{name}: dependency path escapes the workspace"));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn dependency_sources_stay_in_the_workspace_or_registry() {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate_sources(&metadata).unwrap();
}

#[test]
fn default_build_graph_excludes_native_build_tools() {
    // At the workspace root Cargo selects default-members. Include normal edges so
    // build dependencies of transitive dependencies cannot escape the check.
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .args([
            "tree",
            "--locked",
            "--offline",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(!graph.trim().is_empty(), "default build graph is empty");
    for name in graph
        .lines()
        .filter_map(|line| line.split_whitespace().next())
    {
        assert!(
            !["cc", "cmake", "bindgen", "pkg-config"].contains(&name),
            "default build graph contains prohibited native build tool {name}:\n{graph}"
        );
    }
}

#[test]
fn dependency_policy_rejects_external_paths_and_git() {
    let root = workspace();
    let metadata = |source: Value, manifest: PathBuf| {
        serde_json::json!({
            "workspace_root": root.join("crates/protocol"),
            "packages": [{"name": "fixture", "source": source, "manifest_path": manifest}],
        })
    };
    let external_path = metadata(Value::Null, root.join("Cargo.toml"));
    assert!(
        validate_sources(&external_path)
            .unwrap_err()
            .contains("escapes")
    );
    let git_source = metadata(
        Value::String(["git+", "file:", "fixture"].concat()),
        root.join("Cargo.toml"),
    );
    assert!(
        validate_sources(&git_source)
            .unwrap_err()
            .contains("sources")
    );
    let inside = metadata(Value::Null, root.join("crates/protocol/Cargo.toml"));
    assert!(validate_sources(&inside).is_ok());
}

#[test]
fn policy_scripts_pass_their_tests() {
    let root = workspace();
    let output_dir = root.join("target/policy-tests");
    std::fs::create_dir_all(&output_dir).unwrap();
    for (source, name) in [
        ("scripts/content_policy.rs", "content-policy"),
        (".github/scripts/ci_status.rs", "ci-status"),
    ] {
        let binary = output_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        let compile = Command::new("rustc")
            .current_dir(&root)
            .args(["--edition=2024", "-D", "warnings", "--test", source, "-o"])
            .arg(&binary)
            .status()
            .unwrap();
        assert!(compile.success(), "cannot compile {source}");
        assert!(
            Command::new(&binary).status().unwrap().success(),
            "{source} tests failed"
        );
    }
}

#[cfg(unix)]
fn without_git_environment(mut command: Command) -> Command {
    for (name, _) in std::env::vars_os() {
        if name.as_encoded_bytes().starts_with(b"GIT_") {
            command.env_remove(name);
        }
    }
    command
}

#[cfg(unix)]
#[test]
fn push_hook_ignores_inherited_git_environment() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let other = std::env::temp_dir().join(format!(
        "dmd-hook-other-repository-{}-{nonce}",
        std::process::id()
    ));
    let init = without_git_environment(Command::new("git"))
        .args(["init", "--bare"])
        .arg(&other)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    for contaminated in [false, true] {
        let mut test = without_git_environment(Command::new(std::env::current_exe().unwrap()));
        test.args([
            "--exact",
            "push_hook_enforces_content_and_local_policy",
            "--nocapture",
        ]);
        if contaminated {
            test.env("GIT_DIR", &other);
            for name in [
                "GIT_WORK_TREE",
                "GIT_INDEX_FILE",
                "GIT_OBJECT_DIRECTORY",
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                "GIT_COMMON_DIR",
                "GIT_PREFIX",
            ] {
                test.env(name, other.join("unused"));
            }
        }
        let output = test.output().unwrap();
        assert!(
            output.status.success(),
            "hook fixture failed (contaminated={contaminated}):\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}

#[cfg(unix)]
fn hook_fixture() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let root = workspace();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture =
        std::env::temp_dir().join(format!("dmd-hook-policy-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(fixture.join("scripts")).unwrap();
    std::fs::create_dir_all(fixture.join("bin")).unwrap();
    std::fs::copy(
        root.join("scripts/content_policy.rs"),
        fixture.join("scripts/content_policy.rs"),
    )
    .unwrap();
    let cargo = fixture.join("bin/cargo");
    std::fs::write(&cargo, "exit 0\n").unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let git = |args: &[&str]| fixture_git(&fixture, args);
    git(&["init", "--initial-branch=topic"]);
    git(&["config", "user.name", "Policy Test"]);
    git(&["config", "user.email", &["policy", "invalid"].join("@")]);
    fixture
}

#[cfg(unix)]
fn fixture_git(fixture: &Path, args: &[&str]) -> String {
    let output = without_git_environment(Command::new("git"))
        .current_dir(fixture)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[cfg(unix)]
#[test]
fn push_hook_enforces_content_and_local_policy() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = hook_fixture();
    let git = |args: &[&str]| fixture_git(&fixture, args);
    std::fs::write(fixture.join("note.txt"), "Safe content\n").unwrap();
    git(&["add", "note.txt"]);
    git(&["commit", "-m", "Add safe content"]);
    let base = git(&["rev-parse", "HEAD"]);
    let hook = |head: &str, destination: &str| invoke_hook(&fixture, &base, head, destination);
    let clean = hook(&base, "topic");
    assert!(
        clean.status.success(),
        "{}",
        String::from_utf8_lossy(&clean.stderr)
    );
    assert!(String::from_utf8_lossy(&clean.stderr).contains("no local policy hook configured"));
    let main = hook(&base, "main");
    assert!(!main.status.success());
    assert!(String::from_utf8_lossy(&main.stderr).contains("Pull Request"));
    for invalid in ["", "missing-policy", "scripts", "note.txt"] {
        git(&["config", "dmd.localPolicy", invalid]);
        let rejected = hook(&base, "topic");
        assert!(!rejected.status.success(), "accepted policy {invalid:?}");
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(stderr.contains("dmd.localPolicy must name an executable file"));
        assert!(!stderr.contains("no local policy hook configured"));
    }
    git(&["config", "--unset", "dmd.localPolicy"]);
    std::fs::write(fixture.join("note.txt"), ["", "home", "person"].join("/")).unwrap();
    git(&["add", "note.txt"]);
    git(&["commit", "-m", "Add a policy fixture"]);
    let bad = hook(&git(&["rev-parse", "HEAD"]), "topic");
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("private filesystem location"));
    let local = fixture.join("local-policy");
    std::fs::write(&local, "exit 1\n").unwrap();
    std::fs::set_permissions(&local, std::fs::Permissions::from_mode(0o755)).unwrap();
    git(&["config", "dmd.localPolicy", local.to_str().unwrap()]);
    let failed = hook(&base, "topic");
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("local policy hook configured"));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("local policy hook failed"));
    std::fs::write(&local, "exit 0\n").unwrap();
    assert!(hook(&base, "topic").status.success());
    assert_relative_policy_ignores_path_decoy(&fixture, &base);
}

#[cfg(unix)]
fn assert_relative_policy_ignores_path_decoy(fixture: &Path, base: &str) {
    use std::os::unix::fs::PermissionsExt;

    for (relative, marker) in [
        ("local-policy", "configured"),
        ("bin/local-policy", "decoy"),
    ] {
        let script = fixture.join(relative);
        std::fs::write(&script, format!("printf '{marker}' > policy-ran\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for configured in ["local-policy", "./local-policy"] {
        let config = without_git_environment(Command::new("git"))
            .current_dir(fixture)
            .args(["config", "dmd.localPolicy", configured])
            .output()
            .unwrap();
        assert!(
            config.status.success(),
            "{}",
            String::from_utf8_lossy(&config.stderr)
        );
        let result = invoke_hook(fixture, base, base, "topic");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let marker = fixture.join("policy-ran");
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "configured");
        std::fs::remove_file(marker).unwrap();
    }
}

#[cfg(unix)]
fn invoke_hook(fixture: &Path, base: &str, head: &str, destination: &str) -> std::process::Output {
    let path = std::env::join_paths(
        std::iter::once(fixture.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let updates = fixture.join("updates");
    std::fs::write(
        &updates,
        format!("refs/heads/topic {head} refs/heads/{destination} {base}\n"),
    )
    .unwrap();
    without_git_environment(Command::new("git"))
        .current_dir(fixture)
        .args([
            "-c",
            &format!("core.hooksPath={}", workspace().join(".githooks").display()),
            "hook",
            "run",
            &format!("--to-stdin={}", updates.display()),
            "pre-push",
            "--",
            "origin",
            "fixture",
        ])
        .env("PATH", &path)
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn push_hook_caches_only_successful_policy_builds() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = hook_fixture();
    fixture_git(
        &fixture,
        &["commit", "--allow-empty", "-m", "Add safe base"],
    );
    let base = fixture_git(&fixture, &["rev-parse", "HEAD"]);
    let hook = |destination| invoke_hook(&fixture, &base, &base, destination);
    let hash_file = fixture.join("target/policy/content-policy.hash");
    assert!(!hook("main").status.success());
    assert!(!hash_file.exists());
    assert!(hook("topic").status.success());
    let hash = fixture_git(&fixture, &["hash-object", "scripts/content_policy.rs"]);
    assert_eq!(std::fs::read_to_string(&hash_file).unwrap().trim(), hash);

    let compiler = fixture.join("bin/rustc");
    std::fs::write(&compiler, "exit 1\n").unwrap();
    std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(hook("topic").status.success());
    let source = fixture.join("scripts/content_policy.rs");
    let original = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{original}\n")).unwrap();
    let failed = hook("topic");
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("cannot compile the content policy"));
    assert!(!hash_file.exists());

    std::fs::remove_file(&compiler).unwrap();
    assert!(hook("topic").status.success());
    let changed_hash = fixture_git(&fixture, &["hash-object", "scripts/content_policy.rs"]);
    assert_ne!(hash, changed_hash);
    assert_eq!(
        std::fs::read_to_string(&hash_file).unwrap().trim(),
        changed_hash
    );
    std::fs::remove_file(fixture.join("target/policy/content-policy")).unwrap();
    assert!(hook("topic").status.success());
    assert!(fixture.join("target/policy/content-policy").is_file());
}

#[cfg(unix)]
#[test]
fn push_hook_checks_new_branches_against_only_the_destination() {
    let fixture = hook_fixture();
    let git = |args: &[&str]| fixture_git(&fixture, args);
    git(&["remote", "add", "origin", "."]);
    git(&["remote", "add", "secondary", "."]);
    git(&["commit", "--allow-empty", "-m", "Add safe base"]);
    let base = git(&["rev-parse", "HEAD"]);
    std::fs::write(fixture.join("note.txt"), ["", "home", "person"].join("/")).unwrap();
    git(&["add", "note.txt"]);
    git(&["commit", "-m", "Add policy fixture"]);
    let head = git(&["rev-parse", "HEAD"]);
    git(&["update-ref", "refs/remotes/secondary/topic", &head]);
    // A similarly prefixed remote must not count as the destination either.
    git(&["update-ref", "refs/remotes/origin-backup/topic", &head]);
    for destination_has_refs in [false, true] {
        if destination_has_refs {
            git(&["update-ref", "refs/remotes/origin/base", &base]);
        }
        let rejected = invoke_hook(&fixture, &"0".repeat(40), &head, "topic");
        assert!(!rejected.status.success());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("private filesystem location"));
    }
    git(&["update-ref", "refs/remotes/origin/topic", &head]);
    let accepted = invoke_hook(&fixture, &"0".repeat(40), &head, "topic");
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
}

#[cfg(unix)]
#[test]
fn push_hook_checks_files_and_text_introduced_only_by_a_merge() {
    for location in ["filename", "added text"] {
        let fixture = hook_fixture();
        let git = |args: &[&str]| fixture_git(&fixture, args);
        git(&["commit", "--allow-empty", "-m", "Add safe base"]);
        git(&["checkout", "-b", "side"]);
        git(&["commit", "--allow-empty", "-m", "Add side history"]);
        git(&["checkout", "topic"]);
        git(&["commit", "--allow-empty", "-m", "Add topic history"]);
        let base = git(&["rev-parse", "HEAD"]);
        git(&["merge", "--no-ff", "--no-commit", "side"]);
        let address = ["person", &["mail", "invalid"].join(".")].join("@");
        let (name, content) = if location == "filename" {
            (address.as_str(), "Safe content")
        } else {
            ("note.txt", address.as_str())
        };
        std::fs::write(fixture.join(name), content).unwrap();
        git(&["add", name]);
        git(&["commit", "-m", "Merge side history"]);
        let head = git(&["rev-parse", "HEAD"]);
        let rejected = invoke_hook(&fixture, &base, &head, "topic");
        assert!(!rejected.status.success(), "accepted merge {location}");
        assert!(
            String::from_utf8_lossy(&rejected.stderr)
                .contains(&format!("e-mail address in {location}")),
            "{}",
            String::from_utf8_lossy(&rejected.stderr)
        );
    }
}
