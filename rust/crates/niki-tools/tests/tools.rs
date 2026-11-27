use std::fs;
use std::time::{Duration, Instant};

use niki_tools::{
    MAX_OUTPUT_BYTES, MAX_OUTPUT_LINES, Registry, Shell, Tool, ToolContext, ToolError, ToolOutput,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn context(dir: &TempDir) -> ToolContext {
    ToolContext::new(dir.path()).expect("workspace root")
}

async fn call(
    registry: &Registry,
    ctx: &ToolContext,
    name: &str,
    args: Value,
) -> Result<ToolOutput, ToolError> {
    let tool = registry.get(name).expect("tool is registered");
    tool.run(args, ctx).await
}

async fn ok(registry: &Registry, ctx: &ToolContext, name: &str, args: Value) -> ToolOutput {
    call(registry, ctx, name, args)
        .await
        .unwrap_or_else(|error| panic!("{name} failed: {error}"))
}

#[tokio::test]
async fn read_file_honours_offset_and_limit() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("notes.txt"), "one\ntwo\nthree\nfour\n").expect("write");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "read_file",
        json!({"path": "notes.txt", "offset": 2, "limit": 2}),
    )
    .await;

    assert_eq!(output.text, "two\nthree\n");
    assert!(!output.truncated);
}

#[tokio::test]
async fn read_file_marks_a_truncated_large_file() {
    let dir = TempDir::new().expect("temp dir");
    let mut contents = String::new();
    for line in 1..=5_000 {
        contents.push_str(&format!("line {line}\n"));
    }
    fs::write(dir.path().join("large.txt"), contents).expect("write");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(&registry, &ctx, "read_file", json!({"path": "large.txt"})).await;

    assert!(output.truncated);
    assert!(output.text.contains("[output truncated:"));
    assert!(output.text.starts_with("line 1\n"));
    assert!(!output.text.contains("line 5000"));
    assert!(output.text.len() <= MAX_OUTPUT_BYTES);
    let body_lines = output
        .text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with("[output truncated:"))
        .count();
    assert_eq!(body_lines, MAX_OUTPUT_LINES);
}

#[tokio::test]
async fn write_file_creates_nested_directories() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "write_file",
        json!({"path": "a/b/c.txt", "content": "hello"}),
    )
    .await;

    assert!(output.text.contains("a/b/c.txt"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a/b/c.txt")).expect("read back"),
        "hello"
    );
}

#[tokio::test]
async fn edit_file_rejects_a_non_unique_match() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("dup.txt"), "x x\n").expect("write");
    let ctx = context(&dir);
    let registry = Registry::new();

    let result = call(
        &registry,
        &ctx,
        "edit_file",
        json!({"path": "dup.txt", "old": "x", "new": "y"}),
    )
    .await;

    match result {
        Err(ToolError::AmbiguousMatch { count, path }) => {
            assert_eq!(count, 2);
            assert_eq!(path, "dup.txt");
        }
        other => panic!("expected AmbiguousMatch, got {other:?}"),
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("dup.txt")).expect("unchanged"),
        "x x\n"
    );
}

#[tokio::test]
async fn edit_file_replaces_every_match_when_asked() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("dup.txt"), "x x\n").expect("write");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "edit_file",
        json!({"path": "dup.txt", "old": "x", "new": "y", "replace_all": true}),
    )
    .await;

    assert!(output.text.contains("2 replacements"));
    assert_eq!(
        fs::read_to_string(dir.path().join("dup.txt")).expect("edited"),
        "y y\n"
    );
}

#[tokio::test]
async fn list_dir_is_sorted_and_marks_directories() {
    let dir = TempDir::new().expect("temp dir");
    fs::create_dir(dir.path().join("a")).expect("mkdir a");
    fs::create_dir(dir.path().join("c")).expect("mkdir c");
    fs::write(dir.path().join("b.txt"), "").expect("write b");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(&registry, &ctx, "list_dir", json!({"path": "."})).await;

    assert_eq!(output.text, "a/\nb.txt\nc/");
}

#[tokio::test]
async fn shell_reports_exit_code_stdout_and_stderr() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "shell",
        json!({"command": "printf out; printf err >&2; exit 3"}),
    )
    .await;

    assert!(output.text.starts_with("exit_code: 3\n"));
    assert!(output.text.contains("--- stdout ---\nout\n"));
    assert!(output.text.contains("--- stderr ---\nerr\n"));
}

#[tokio::test]
async fn shell_timeout_kills_a_long_command() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let shell = Shell::with_timeout(Duration::from_millis(200));

    let started = Instant::now();
    let result = Tool::run(&shell, json!({"command": "sleep 5"}), &ctx).await;
    let elapsed = started.elapsed();

    match result {
        Err(ToolError::Timeout { command, .. }) => assert_eq!(command, "sleep 5"),
        other => panic!("expected Timeout, got {other:?}"),
    }
    assert!(
        elapsed < Duration::from_secs(4),
        "the child was not killed promptly: {elapsed:?}"
    );
}

#[tokio::test]
async fn shell_finishes_and_stays_bounded_when_a_command_floods_output() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let shell = Shell::with_timeout(Duration::from_secs(30));

    let started = Instant::now();
    let result = Tool::run(
        &shell,
        json!({"command": "yes niki | head -c 4000000"}),
        &ctx,
    )
    .await;
    let elapsed = started.elapsed();

    let output = result.expect("a flooding command still finishes");
    assert!(
        output.truncated,
        "the captured text is reported as cut: {} bytes",
        output.text.len()
    );
    assert!(
        output.text.len() <= MAX_OUTPUT_BYTES + 128,
        "captured text stays at the cap: {} bytes",
        output.text.len()
    );
    assert!(
        output
            .text
            .contains("--- stdout (truncated, first 65536 bytes) ---"),
        "the truncation is visible: {:?}",
        &output.text[..output.text.len().min(200)]
    );
    assert!(
        elapsed < Duration::from_secs(20),
        "the flood was drained promptly: {elapsed:?}"
    );
}

#[tokio::test]
async fn grep_finds_matches_and_skips_noise_directories() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).expect("mkdir src");
    fs::write(root.join("src/main.rs"), "fn main() {}\nlet needle = 42;\n").expect("write main");
    for skipped in [".git", "node_modules", "target", ".venv", "__pycache__"] {
        let nested = root.join(skipped).join("nested");
        fs::create_dir_all(&nested).expect("mkdir skipped");
        fs::write(nested.join("noise.txt"), "needle\n").expect("write noise");
    }
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(&registry, &ctx, "grep", json!({"pattern": "needle\\s*="})).await;

    assert_eq!(output.text, "src/main.rs:2:let needle = 42;");
    assert!(!output.text.contains("node_modules"));
    assert!(!output.text.contains(".git"));
    assert!(!output.text.contains("target"));
    assert!(!output.text.contains(".venv"));
    assert!(!output.text.contains("__pycache__"));

    let scoped = ok(
        &registry,
        &ctx,
        "grep",
        json!({"pattern": "needle", "path": "src", "glob": "**/*.rs"}),
    )
    .await;
    assert_eq!(scoped.text, "src/main.rs:2:let needle = 42;");
}

#[tokio::test]
async fn glob_matches_relative_paths_sorted() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("b")).expect("mkdir b");
    fs::write(root.join("a.rs"), "").expect("write a.rs");
    fs::write(root.join("b/c.rs"), "").expect("write c.rs");
    fs::write(root.join("d.txt"), "").expect("write d.txt");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(&registry, &ctx, "glob", json!({"pattern": "**/*.rs"})).await;

    assert_eq!(output.text, "a.rs\nb/c.rs");
}

#[tokio::test]
async fn paths_escaping_the_root_are_refused() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let registry = Registry::new();

    let read = call(
        &registry,
        &ctx,
        "read_file",
        json!({"path": "../outside.txt"}),
    )
    .await;
    assert!(matches!(read, Err(ToolError::OutsideRoot { .. })));

    let write = call(
        &registry,
        &ctx,
        "write_file",
        json!({"path": "../escape.txt", "content": "nope"}),
    )
    .await;
    assert!(matches!(write, Err(ToolError::OutsideRoot { .. })));

    let absolute = call(&registry, &ctx, "read_file", json!({"path": "/etc/passwd"})).await;
    assert!(matches!(absolute, Err(ToolError::OutsideRoot { .. })));

    fs::create_dir(dir.path().join("sub")).expect("mkdir sub");
    let through_existing = call(
        &registry,
        &ctx,
        "write_file",
        json!({"path": "sub/../../escape.txt", "content": "nope"}),
    )
    .await;
    assert!(matches!(
        through_existing,
        Err(ToolError::OutsideRoot { .. })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_staying_inside_the_root_is_allowed() {
    let dir = TempDir::new().expect("temp dir");
    fs::create_dir(dir.path().join("real")).expect("mkdir real");
    fs::write(dir.path().join("real/x.txt"), "inside").expect("write x");
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("alias")).expect("symlink");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(&registry, &ctx, "read_file", json!({"path": "alias/x.txt"})).await;

    assert_eq!(output.text, "inside");
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_leaving_the_root_is_refused() {
    let dir = TempDir::new().expect("temp dir");
    let outside = TempDir::new().expect("outside dir");
    fs::write(outside.path().join("secret.txt"), "top secret").expect("write secret");
    std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).expect("symlink");
    let ctx = context(&dir);
    let registry = Registry::new();

    let result = call(
        &registry,
        &ctx,
        "read_file",
        json!({"path": "link/secret.txt"}),
    )
    .await;

    assert!(matches!(result, Err(ToolError::OutsideRoot { .. })));
}

#[tokio::test]
async fn registry_specs_cover_every_tool() {
    let registry = Registry::new();
    let specs = registry.specs();
    let specs = specs.as_array().expect("specs is an array");

    let names: Vec<&str> = specs
        .iter()
        .filter_map(|spec| spec.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(
        names,
        [
            "edit_file",
            "glob",
            "grep",
            "list_dir",
            "read_file",
            "request_user_input",
            "shell",
            "write_file"
        ]
    );

    for spec in specs {
        assert!(
            spec.get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| !description.is_empty())
        );
        let parameters = spec.get("parameters").expect("parameters present");
        assert_eq!(
            parameters.get("type").and_then(Value::as_str),
            Some("object")
        );
        assert!(parameters.get("properties").is_some_and(Value::is_object));
    }

    assert!(serde_json::to_string(&specs).is_ok());
    assert!(registry.get("no_such_tool").is_none());
}

#[tokio::test]
async fn write_file_accepts_the_wire_file_path_alias() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "write_file",
        json!({"file_path": "alias.txt", "content": "hello"}),
    )
    .await;

    assert!(output.text.contains("alias.txt"));
    assert_eq!(
        fs::read_to_string(dir.path().join("alias.txt")).expect("read back"),
        "hello"
    );
}

#[tokio::test]
async fn edit_file_accepts_the_wire_string_aliases() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("alias.txt"), "one two\n").expect("write");
    let ctx = context(&dir);
    let registry = Registry::new();

    ok(
        &registry,
        &ctx,
        "edit_file",
        json!({"file_path": "alias.txt", "old_string": "one", "new_string": "three"}),
    )
    .await;

    assert_eq!(
        fs::read_to_string(dir.path().join("alias.txt")).expect("read back"),
        "three two\n"
    );
}

#[tokio::test]
async fn read_file_accepts_the_wire_file_path_alias() {
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join("alias.txt"), "alias body\n").expect("write");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "read_file",
        json!({"file_path": "alias.txt"}),
    )
    .await;

    assert!(output.text.contains("alias body"));
}

#[tokio::test]
async fn the_schemas_advertise_the_canonical_argument_names() {
    let registry = Registry::new();

    for (tool, required) in [
        ("read_file", vec!["file_path"]),
        ("write_file", vec!["file_path", "content"]),
        ("edit_file", vec!["file_path", "old_string", "new_string"]),
    ] {
        let schema = registry.get(tool).expect("tool is registered").schema();
        let names: Vec<&str> = schema["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(names, required, "{tool} required list");
        for property in required {
            assert!(
                schema["properties"][property].is_object(),
                "{tool} does not describe {property}"
            );
        }
    }
}

#[tokio::test]
async fn write_file_reports_the_path_the_model_named() {
    let dir = TempDir::new().expect("temp dir");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(
        &registry,
        &ctx,
        "write_file",
        json!({"file_path": "src/notes.txt", "content": "body"}),
    )
    .await;

    assert_eq!(output.text, "Updated file src/notes.txt");
}

#[tokio::test]
async fn edit_file_refuses_a_file_larger_than_it_will_load() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("huge.bin");
    let file = fs::File::create(&path).expect("create");
    file.set_len(40 * 1024 * 1024).expect("sparse size");
    drop(file);
    let ctx = context(&dir);
    let registry = Registry::new();

    let error = call(
        &registry,
        &ctx,
        "edit_file",
        json!({"file_path": "huge.bin", "old_string": "a", "new_string": "b"}),
    )
    .await
    .expect_err("the edit is refused instead of loading 40 MiB");

    match error {
        ToolError::InvalidArguments(message) => {
            assert!(message.contains("huge.bin"), "{message}");
            assert!(message.contains("the limit is"), "{message}");
        }
        other => panic!("expected InvalidArguments, got {other:?}"),
    }
}

#[tokio::test]
async fn grep_skips_a_file_larger_than_it_will_load() {
    use std::io::Write;

    let dir = TempDir::new().expect("temp dir");
    let mut contents = vec![b'a'; 40 * 1024 * 1024];
    contents[..6].copy_from_slice(b"needle");
    let mut file = fs::File::create(dir.path().join("blob.txt")).expect("create");
    file.write_all(&contents).expect("write");
    drop(file);
    drop(contents);
    fs::write(dir.path().join("small.txt"), "needle here\n").expect("write small");
    let ctx = context(&dir);
    let registry = Registry::new();

    let output = ok(&registry, &ctx, "grep", json!({"pattern": "needle"})).await;

    assert!(
        output.text.contains("small.txt:1:needle"),
        "the readable file is searched: {}",
        output.text
    );
    assert!(
        !output.text.contains("blob.txt"),
        "the oversized file is not loaded: {}",
        output.text
    );
}
