//! What `/diff` puts in the transcript, driven through real git repositories.

use std::path::Path;
use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use niki_policy::Mode;
use niki_tui_live::{App, RowKind, SessionInfo, TurnState, draw};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    git(temp.path(), &["init", "-q"]);
    temp
}

/// Run git in `root` with an identity on the command line, so no global
/// configuration is needed.
fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.email=test@example.com"])
        .args(["-c", "user.name=Test"])
        .args(arguments)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {arguments:?} failed");
}

fn app_at(root: &Path) -> App {
    App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: root.to_path_buf(),
    })
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
}

fn run(app: &mut App, line: &str) {
    type_text(app, line);
    assert_eq!(
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        None
    );
}

fn texts(app: &App) -> Vec<String> {
    app.transcript
        .rows()
        .iter()
        .map(|row| match &row.title {
            Some(title) => format!("{title}: {}", row.text),
            None => row.text.clone(),
        })
        .collect()
}

fn diff_titles(app: &App) -> Vec<String> {
    app.transcript
        .rows()
        .iter()
        .filter(|row| row.kind == RowKind::Diff)
        .map(|row| row.title.clone().unwrap_or_default())
        .collect()
}

#[test]
fn a_two_hunk_file_is_one_titled_row_with_both_hunks() {
    let temp = repo();
    let root = temp.path();
    let original: String = (0..30).map(|line| format!("line {line}\n")).collect();
    std::fs::write(root.join("main.rs"), &original).expect("write");
    git(root, &["add", "main.rs"]);
    git(root, &["commit", "-qm", "base"]);

    let mut changed: Vec<String> = (0..30).map(|line| format!("line {line}")).collect();
    changed[0] = "first changed".to_owned();
    changed[29] = "last changed".to_owned();
    std::fs::write(root.join("main.rs"), changed.join("\n") + "\n").expect("write");

    let mut app = app_at(root);
    run(&mut app, "/diff unstaged");

    assert_eq!(diff_titles(&app), ["main.rs"]);
    let row = app
        .transcript
        .rows()
        .iter()
        .find(|row| row.kind == RowKind::Diff)
        .expect("a diff row");
    let rendered = row.rendered.as_ref().expect("rendered lines");
    assert_eq!(rendered.len(), 12, "two headers and ten hunk lines");
    let headers = rendered
        .iter()
        .filter(|line| line.kind == niki_render::LineKind::DiffHunk)
        .count();
    assert_eq!(headers, 2);
    assert!(
        texts(&app).iter().any(|text| text == "1 file, +2 −2"),
        "{:?}",
        texts(&app)
    );
}

#[test]
fn diff_staged_includes_a_staged_change_and_excludes_an_unstaged_one() {
    let temp = repo();
    let root = temp.path();
    std::fs::write(root.join("tracked.txt"), "base\n").expect("write");
    git(root, &["add", "tracked.txt"]);
    git(root, &["commit", "-qm", "base"]);

    std::fs::write(root.join("staged.txt"), "staged\n").expect("write");
    git(root, &["add", "staged.txt"]);
    std::fs::write(root.join("tracked.txt"), "changed\n").expect("write");

    let mut staged = app_at(root);
    run(&mut staged, "/diff staged");
    assert_eq!(diff_titles(&staged), ["staged.txt"]);
    assert!(texts(&staged).iter().any(|text| text == "1 file, +1 −0"));

    let mut unstaged = app_at(root);
    run(&mut unstaged, "/diff unstaged");
    assert_eq!(diff_titles(&unstaged), ["tracked.txt"]);
    assert!(
        texts(&unstaged).iter().any(|text| text == "1 file, +1 −1"),
        "{:?}",
        texts(&unstaged)
    );
}

#[test]
fn no_argument_shows_every_section_under_its_own_heading() {
    let temp = repo();
    let root = temp.path();
    std::fs::write(root.join("tracked.txt"), "base\n").expect("write");
    git(root, &["add", "tracked.txt"]);
    git(root, &["commit", "-qm", "base"]);

    std::fs::write(root.join("tracked.txt"), "staged\n").expect("write");
    git(root, &["add", "tracked.txt"]);
    std::fs::write(root.join("tracked.txt"), "unstaged\n").expect("write");
    std::fs::write(root.join("new.txt"), "new\n").expect("write");

    let mut app = app_at(root);
    run(&mut app, "/diff");

    let all = texts(&app);
    for heading in ["staged", "unstaged", "untracked"] {
        assert!(all.iter().any(|text| text == heading), "{heading}: {all:?}");
    }
    assert!(diff_titles(&app).contains(&"tracked.txt".to_owned()));
    assert!(diff_titles(&app).contains(&"new.txt".to_owned()));
}

#[test]
fn the_line_cap_keeps_four_hundred_and_says_what_it_withheld() {
    let temp = repo();
    let content: String = (0..500).map(|line| format!("line {line}\n")).collect();
    std::fs::write(temp.path().join("big.txt"), content).expect("write");

    let mut app = app_at(temp.path());
    run(&mut app, "/diff untracked");

    let row = app
        .transcript
        .rows()
        .iter()
        .find(|row| row.kind == RowKind::Diff)
        .expect("a diff row");
    assert_eq!(row.rendered.as_ref().expect("lines").len(), 400);
    assert!(
        texts(&app)
            .iter()
            .any(|text| text == "101 more lines withheld"),
        "{:?}",
        texts(&app)
    );
    assert!(
        texts(&app).iter().any(|text| text == "1 file, +500 −0"),
        "{:?}",
        texts(&app)
    );
}

#[test]
fn the_file_cap_keeps_twenty_and_says_what_it_withheld() {
    let temp = repo();
    for index in 0..21 {
        std::fs::write(temp.path().join(format!("f{index}.txt")), "x\n").expect("write");
    }

    let mut app = app_at(temp.path());
    run(&mut app, "/diff untracked");

    assert_eq!(diff_titles(&app).len(), 20);
    assert!(
        texts(&app)
            .iter()
            .any(|text| text == "1 more file and 2 more lines withheld"),
        "{:?}",
        texts(&app)
    );
    assert!(
        texts(&app).iter().any(|text| text == "21 files, +21 −0"),
        "{:?}",
        texts(&app)
    );
}

#[test]
fn a_clean_tree_prints_no_changes_and_nothing_else() {
    let temp = repo();
    let mut app = app_at(temp.path());
    run(&mut app, "/diff");

    assert_eq!(app.transcript.rows().len(), 1);
    assert_eq!(app.transcript.rows()[0].kind, RowKind::Notice);
    assert_eq!(app.transcript.rows()[0].text, "no changes");
}

#[test]
fn a_turn_in_flight_refuses_the_diff() {
    let temp = repo();
    let mut app = app_at(temp.path());
    app.status.turn = TurnState::Working;

    run(&mut app, "/diff");

    assert!(diff_titles(&app).is_empty());
    let last = app.transcript.rows().last().expect("a notice");
    assert!(
        last.text.contains("/diff waits until the turn finishes"),
        "{}",
        last.text
    );
}

#[test]
fn a_directory_outside_a_repository_is_one_line() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = app_at(temp.path());
    run(&mut app, "/diff");

    assert_eq!(app.transcript.rows().len(), 1);
    let last = &app.transcript.rows()[0];
    assert_eq!(last.kind, RowKind::Notice);
    assert!(
        last.text.starts_with("not a git repository: "),
        "{}",
        last.text
    );
    assert_eq!(last.text.lines().count(), 1);
}

#[test]
fn an_unknown_section_names_the_three_it_has() {
    let temp = repo();
    let mut app = app_at(temp.path());
    run(&mut app, "/diff nothing");

    let last = app.transcript.rows().last().expect("a notice");
    assert!(
        last.text
            .contains("there is no nothing section; try staged, unstaged, untracked"),
        "{}",
        last.text
    );
}

#[test]
fn the_screen_paints_the_path_heading_and_diff_lines() {
    let temp = repo();
    std::fs::write(temp.path().join("hello.txt"), "hello\n").expect("write");
    let mut app = app_at(temp.path());
    run(&mut app, "/diff untracked");

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let mut screen = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            screen.push_str(buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()));
        }
        screen.push('\n');
    }
    assert!(screen.contains("untracked"), "{screen}");
    assert!(screen.contains("hello.txt"), "{screen}");
    assert!(screen.contains("+hello"), "{screen}");
}
