//! Root detection, ignore filtering, and index limits.

use std::fs;
use std::path::{Path, PathBuf};

use niki_workspace::{Root, WorkspaceError, detect_root};
use tempfile::TempDir;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

fn marked_project(temp: &TempDir) -> PathBuf {
    let root = temp.path().join("project");
    fs::create_dir_all(root.join(".git")).unwrap();
    root
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap()
}

fn names(root: &Root) -> Vec<String> {
    root.files()
        .unwrap()
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn detects_the_root_above_a_nested_file() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    let nested = root.join("src").join("deep").join("main.rs");
    write(&nested, "fn main() {}\n");

    assert_eq!(detect_root(&nested), Some(canonical(&root)));
    assert_eq!(detect_root(root.join("src")), Some(canonical(&root)));
    assert_eq!(detect_root(&root), Some(canonical(&root)));
}

#[test]
fn a_niki_directory_marks_a_root_without_git() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir_all(root.join(".niki")).unwrap();

    let detected = Root::detect(&root).unwrap();
    assert_eq!(detected.path(), canonical(&root));
}

#[test]
fn no_marker_above_the_start_returns_none() {
    let temp = TempDir::new().unwrap();
    let plain = temp.path().join("plain").join("inner");
    fs::create_dir_all(&plain).unwrap();

    assert_eq!(detect_root(&plain), None);
    assert!(Root::detect(&plain).is_none());
}

#[test]
fn gitignore_and_built_in_names_filter_the_index() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    write(
        &root.join(".gitignore"),
        "# build output\n*.log\nlogs/\n/root-only.txt\nnested/ignored.txt\n!keep.log\n",
    );
    write(&root.join("README.md"), "readme\n");
    write(&root.join("keep.txt"), "keep\n");
    write(&root.join("drop.log"), "dropped\n");
    write(&root.join("negated.log"), "negation is not supported\n");
    write(&root.join("root-only.txt"), "dropped\n");
    write(&root.join("logs").join("today.txt"), "dropped\n");
    write(&root.join("nested").join("ignored.txt"), "dropped\n");
    write(&root.join("nested").join("keep.rs"), "fn keep() {}\n");
    write(&root.join("build").join("out.js"), "dropped\n");
    write(
        &root.join("node_modules").join("pkg").join("index.js"),
        "x\n",
    );
    write(&root.join("target").join("debug").join("niki"), "x\n");
    write(&root.join(".venv").join("bin").join("activate"), "x\n");
    write(&root.join("__pycache__").join("mod.pyc"), "x\n");
    write(&root.join("dist").join("bundle.js"), "x\n");

    let workspace = Root::detect(&root).unwrap();
    assert_eq!(
        names(&workspace),
        [".gitignore", "README.md", "keep.txt", "nested/keep.rs",]
    );
}

#[test]
fn files_are_sorted_and_symlinks_are_not_followed() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    let outside = temp.path().join("outside.rs");
    write(&outside, "fn outside() {}\n");
    write(&root.join("zebra.txt"), "z\n");
    write(&root.join("alpha.txt"), "a\n");
    write(&root.join("nested").join("beta.txt"), "b\n");

    let workspace = Root::detect(&root).unwrap();
    assert_eq!(
        names(&workspace),
        ["alpha.txt", "nested/beta.txt", "zebra.txt"]
    );

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, root.join("link.rs")).unwrap();
        std::os::unix::fs::symlink(temp.path(), root.join("linked-dir")).unwrap();
        assert_eq!(workspace.refresh().unwrap(), 3);
        assert_eq!(
            names(&workspace),
            ["alpha.txt", "nested/beta.txt", "zebra.txt"]
        );
    }
}

#[test]
fn grep_returns_path_line_and_text_within_the_limit() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    write(
        &root.join("src").join("lib.rs"),
        "// todo one\nfn a() {}\n// todo two\n",
    );
    write(
        &root.join("src").join("main.rs"),
        "fn main() {}\n// todo three\n",
    );
    write(&root.join("binary.bin"), "todo but not text\n");

    let workspace = Root::detect(&root).unwrap();
    let matches = workspace.grep("todo? (?:one|two|three)", 10).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].path, Path::new("src/lib.rs"));
    assert_eq!(matches[0].line, 1);
    assert_eq!(matches[0].text, "// todo one");
    assert_eq!(matches[1].line, 3);
    assert_eq!(matches[2].path, Path::new("src/main.rs"));

    let bounded = workspace.grep("todo", 2).unwrap();
    assert_eq!(bounded.len(), 2);
    assert!(workspace.grep("todo", 0).unwrap().is_empty());
}

#[test]
fn grep_skips_files_that_are_not_utf8() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    fs::write(root.join("blob.bin"), [0xff_u8, 0xfe, b'n', b'e']).unwrap();
    write(&root.join("note.txt"), "needle\n");

    let workspace = Root::detect(&root).unwrap();
    let matches = workspace.grep("needle", 10).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, Path::new("note.txt"));
}

#[test]
fn glob_matches_root_relative_paths_within_the_limit() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    write(&root.join("a.rs"), "fn a() {}\n");
    write(&root.join("src").join("b.rs"), "fn b() {}\n");
    write(&root.join("src").join("deep").join("c.rs"), "fn c() {}\n");
    write(&root.join("src").join("d.txt"), "text\n");

    let workspace = Root::detect(&root).unwrap();
    assert_eq!(workspace.glob("*.rs", 10).unwrap(), [PathBuf::from("a.rs")]);
    assert_eq!(
        workspace.glob("**/*.rs", 10).unwrap(),
        [
            PathBuf::from("a.rs"),
            PathBuf::from("src/b.rs"),
            PathBuf::from("src/deep/c.rs"),
        ]
    );
    assert_eq!(
        workspace.glob("**/*.rs", 2).unwrap(),
        [PathBuf::from("a.rs"), PathBuf::from("src/b.rs")]
    );
    assert!(workspace.glob("**/*.rs", 0).unwrap().is_empty());
}

#[test]
fn invalid_patterns_are_reported_not_ignored() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    let workspace = Root::detect(&root).unwrap();

    assert!(matches!(
        workspace.grep("(", 1),
        Err(WorkspaceError::Regex(_))
    ));
    assert!(matches!(
        workspace.glob("[", 1),
        Err(WorkspaceError::Glob(_))
    ));
}

#[test]
fn refresh_rebuilds_the_index_and_the_first_read_builds_it_once() {
    let temp = TempDir::new().unwrap();
    let root = marked_project(&temp);
    write(&root.join("one.txt"), "1\n");

    let workspace = Root::detect(&root).unwrap();
    assert_eq!(names(&workspace), ["one.txt"]);

    write(&root.join("two.txt"), "2\n");
    assert_eq!(names(&workspace), ["one.txt"], "the index is cached");

    assert_eq!(workspace.refresh().unwrap(), 2);
    assert_eq!(names(&workspace), ["one.txt", "two.txt"]);
}

#[test]
fn a_root_that_disappears_is_an_io_error() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("gone");
    fs::create_dir_all(root.join(".git")).unwrap();
    let workspace = Root::detect(&root).unwrap();
    fs::remove_dir_all(&root).unwrap();

    assert!(matches!(
        workspace.refresh(),
        Err(WorkspaceError::Io { .. })
    ));
}
