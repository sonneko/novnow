//! CLI（§14、`PLANS.md` §5.7 M6）。起動の判定（§14.3）・起動前の検査と表示（§14.2）・`new`（§14.5）・
//! `check`・終了コード（§14.2.5）。
//!
//! 大半は `novnow::cli::run` をプロセス内で走らせる（出力はバッファ、環境は偽の `Env`、カレント
//! ディレクトリは引数で渡す。**プロセスの環境変数もカレントディレクトリも書き換えない**）。
//! 末尾の `binary_*` は実際の実行ファイル（`CARGO_BIN_EXE_novnow`）を、環境変数を空にした子プロセス
//! として走らせる（終了コードと標準出力・標準エラーの分かれ方を本物で確かめる）。
//!
//! §14.2.2 / §14.2.3 / §14.2.4 の出力例は**仕様書を読み直して一字一句**突き合わせる。

mod sandbox;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};

use novnow::adapters::project_load::is_document;
use novnow::cli::{
    self, decide, display_path, shell_quote, ArgError, Cli, Command, Exit, Launch, Outcome,
    Terminal,
};
use novnow::core::model::project::PROJECT_STRUCTURE_DOC_URL;
use novnow::core::policy::ProjectPath;
use novnow::core::template;
use novnow::ports::env::Os;

use sandbox::{minimal_project, tree, FakeEnv, TempDir};

const SPEC: &str = include_str!("../SPECIFICATIONS.md");

// ============================================================ 共通

/// 1 回の実行の結果。
#[derive(Debug)]
struct Ran {
    outcome: Outcome,
    out: String,
    err: String,
}

impl Ran {
    fn exit(&self) -> Exit {
        match &self.outcome {
            Outcome::Exit(exit) => *exit,
            other => panic!("終了コードではない: {other:?}"),
        }
    }
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// 端末につながっていない状態で走らせる。
fn run(cwd: &Path, env: &FakeEnv, args: &[&str]) -> Ran {
    run_on(cwd, env, Terminal::default(), args)
}

fn run_on(cwd: &Path, env: &FakeEnv, terminal: Terminal, args: &[&str]) -> Ran {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = {
        let mut cli = Cli::new(env, &mut out, &mut err, cwd.to_path_buf(), terminal);
        cli::run(&os_args(args), &mut cli)
    };
    Ran {
        outcome,
        out: String::from_utf8(out).expect("stdout は UTF-8"),
        err: String::from_utf8(err).expect("stderr は UTF-8"),
    }
}

/// 履歴の置き場所を砂場の中にした日本語の環境。
fn env_ja(dir: &TempDir) -> FakeEnv {
    FakeEnv::with_history(&dir.join("state/recent.json")).var("NOVNOW_LANG", "ja")
}

fn env_en(dir: &TempDir) -> FakeEnv {
    FakeEnv::with_history(&dir.join("state/recent.json")).var("NOVNOW_LANG", "en")
}

/// 仕様書の見出し `heading` の直後のコードブロック。
fn spec_block(heading: &str) -> String {
    let start = SPEC
        .find(heading)
        .unwrap_or_else(|| panic!("{heading} が無い"));
    let rest = &SPEC[start..];
    let fence = rest.find("```").expect("コードブロック");
    let open = fence + rest[fence..].find('\n').expect("言語名の行") + 1;
    let close = open + rest[open..].find("```").expect("閉じ");
    rest[open..close].to_owned()
}

/// §14.2.2 の例と同じ不足（project.json / characters.md / plots/all.txt）を持つディレクトリ。
fn spec_like_incomplete(dir: &TempDir, name: &str) -> PathBuf {
    dir.write(
        &format!("{name}/words.md"),
        "# 星図\ndefinition: 古い星図\n",
    );
    dir.write(
        &format!("{name}/materials/index.json"),
        template::MATERIALS_INDEX,
    );
    dir.mkdir(&format!("{name}/contents"));
    dir.mkdir(&format!("{name}/plots"));
    dir.join(name)
}

// ============================================================ 判定（§14.3）

#[test]
fn the_launch_decision_follows_the_fixed_order() {
    let none = Terminal::default();
    let tty = Terminal {
        stdin: true,
        stdout: true,
        stderr: true,
    };
    let decide_args = |args: &[&str], terminal| decide(&os_args(args), terminal);
    let open = |path: &str| Launch::Open(PathBuf::from(path));
    let command = Launch::Command;
    let error = Launch::Error;
    let cases: Vec<(&[&str], Terminal, Launch)> = vec![
        // 引数なし: 端末なら使い方（2）、そうでなければ GUI
        (&[], tty, Launch::Usage),
        (
            &[],
            Terminal {
                stdin: true,
                ..none
            },
            Launch::Usage,
        ),
        (
            &[],
            Terminal {
                stdout: true,
                ..none
            },
            Launch::Usage,
        ),
        (
            &[],
            Terminal {
                stderr: true,
                ..none
            },
            Launch::Gui,
        ),
        (&[], none, Launch::Gui),
        // --gui はどこにあっても最優先
        (&["--gui"], tty, Launch::Gui),
        (&["./novel", "--gui"], tty, Launch::Gui),
        (&["new", "--gui"], tty, Launch::Gui),
        (&["--gui", "--help"], tty, Launch::Gui),
        // --help / --version
        (&["--help"], none, Launch::Help),
        (&["-h"], none, Launch::Help),
        (&["check", "--help"], none, Launch::Help),
        (&["--help", "--version"], none, Launch::Help),
        (&["--version"], none, Launch::Version),
        (&["-V"], none, Launch::Version),
        // サブコマンドは第 1 引数の完全一致だけ
        (&["new"], none, command(Command::New(PathBuf::from(".")))),
        (
            &["new", "a"],
            none,
            command(Command::New(PathBuf::from("a"))),
        ),
        (
            &["check", "a"],
            none,
            command(Command::Check(PathBuf::from("a"))),
        ),
        (
            &["migrate", "a"],
            none,
            command(Command::Migrate(PathBuf::from("a"))),
        ),
        (
            &["mcp", "a"],
            none,
            command(Command::Mcp(PathBuf::from("a"))),
        ),
        (
            &["export", "a", "--range", "@01-04", "--out", "-"],
            none,
            command(Command::Export {
                dir: PathBuf::from("a"),
                options: os_args(&["--range", "@01-04", "--out", "-"]),
            }),
        ),
        (&["./new"], none, open("./new")),
        (&["New"], none, open("New")),
        (&["chec"], none, open("chec")),
        (&["new/"], none, open("new/")),
        (&["-"], none, open("-")),
        (&["./novel"], tty, open("./novel")),
        // 引数の誤り（2）
        (&["check"], none, error(ArgError::MissingDir("check"))),
        (&["migrate"], none, error(ArgError::MissingDir("migrate"))),
        (&["mcp"], none, error(ArgError::MissingDir("mcp"))),
        (&["export"], none, error(ArgError::MissingDir("export"))),
        (
            &["export", "--range", "@01"],
            none,
            error(ArgError::MissingDir("export")),
        ),
        (
            &["new", "a", "b"],
            none,
            error(ArgError::UnexpectedArgument("b".to_owned())),
        ),
        (
            &["check", "a", "b"],
            none,
            error(ArgError::UnexpectedArgument("b".to_owned())),
        ),
        (
            &["new", "-x"],
            none,
            error(ArgError::UnknownOption("-x".to_owned())),
        ),
        (
            &["check", "a", "--state"],
            none,
            error(ArgError::UnknownOption("--state".to_owned())),
        ),
        (
            &["-x"],
            none,
            error(ArgError::UnknownOption("-x".to_owned())),
        ),
        (
            &["--init", "a"],
            none,
            error(ArgError::UnknownOption("--init".to_owned())),
        ),
        (
            &["a", "b"],
            none,
            error(ArgError::UnexpectedArgument("b".to_owned())),
        ),
    ];
    for (args, terminal, want) in cases {
        assert_eq!(decide_args(args, terminal), want, "{args:?} {terminal:?}");
    }
}

#[cfg(unix)]
#[test]
fn a_directory_name_that_is_not_utf8_is_still_a_directory() {
    use std::os::unix::ffi::OsStringExt;
    let raw = OsString::from_vec(vec![b'n', 0xff]);
    assert_eq!(
        decide(std::slice::from_ref(&raw), Terminal::default()),
        Launch::Open(PathBuf::from(raw))
    );
}

// ============================================================ 起動前の検査（§14.2）

#[test]
fn an_empty_directory_prints_every_missing_item_and_exits_3_without_writing() {
    // PLANS.md §5.7 の完了条件「空ディレクトリで novnow . → エラー ＋ exit 3。GUI を出さない」
    let dir = TempDir::new("cli-empty");
    let root = dir.mkdir("novel");
    let before = tree(dir.path());
    let ran = run(&root, &env_ja(&dir), &["."]);
    assert_eq!(ran.outcome, Outcome::Exit(Exit::Project));
    assert_eq!(Exit::Project.code(), 3);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        format!(
            "novnow: このディレクトリは novnow プロジェクトではありません\n  対象: {}\n\n不足しているもの:\n  - project.json\n  - characters.md\n  - words.md\n  - materials/index.json\n  - plots/all.txt\n  - contents/\n  - plots/\n\nプロジェクト構造: {PROJECT_STRUCTURE_DOC_URL}\nこのディレクトリに新規作成する場合: novnow new\n",
            root.display()
        )
    );
    // 他には何もしない（§14.2.3）: ファイルもディレクトリも履歴も作らない
    assert_eq!(tree(dir.path()), before);
    assert!(!dir.join("state").exists());
    assert_eq!(std::fs::read_dir(&root).expect("read_dir").count(), 0);
}

#[test]
fn the_incomplete_message_is_the_example_of_section_14_2_2_and_14_2_3() {
    let dir = TempDir::new("cli-spec-incomplete");
    let root = spec_like_incomplete(&dir, "untitled");
    let shown = display_path(&root);
    for (env, heading) in [
        (env_ja(&dir), "#### 14.2.2 出力（ja）"),
        (env_en(&dir), "#### 14.2.3 出力（en）"),
    ] {
        let ran = run(&root, &env, &["."]);
        assert_eq!(ran.exit(), Exit::Project);
        let want = spec_block(heading).replace("/home/user/novels/untitled", &shown);
        assert_eq!(ran.err, want, "{heading}");
        // `novnow <dir>` を絶対パスで渡しても同じ（カレントディレクトリが対象なら `novnow new` だけ）
        let ran = run(&root, &env, &[&root.to_string_lossy()]);
        assert_eq!(ran.err, want, "{heading}（絶対パス）");
    }
}

#[test]
fn the_new_hint_names_the_directory_when_it_is_not_the_current_one() {
    let dir = TempDir::new("cli-hint");
    let root = spec_like_incomplete(&dir, "novels/星屑 の時計");
    let ran = run(dir.path(), &env_ja(&dir), &["novels/星屑 の時計"]);
    assert_eq!(ran.exit(), Exit::Project);
    let last = ran.err.lines().last().expect("行");
    assert_eq!(
        last,
        format!(
            "このディレクトリに新規作成する場合: novnow new '{}'",
            root.display()
        )
    );
    assert!(ran.err.contains(&format!("  対象: {}\n", root.display())));
}

#[test]
fn a_missing_directory_is_exit_3_and_is_not_created() {
    let dir = TempDir::new("cli-missing");
    let before = tree(dir.path());
    let ran = run(dir.path(), &env_ja(&dir), &["nope"]);
    assert_eq!(ran.exit(), Exit::Project);
    let target = dir.join("nope");
    assert_eq!(
        ran.err,
        format!(
            "novnow: ディレクトリが見つかりません\n  対象: {}\n\nプロジェクト構造: {PROJECT_STRUCTURE_DOC_URL}\nこのディレクトリに新規作成する場合: novnow new {}\n",
            target.display(),
            target.display()
        )
    );
    assert!(!target.exists());
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn a_file_is_not_a_project() {
    let dir = TempDir::new("cli-file");
    let file = dir.write("novel.txt", "本文\n");
    let ran = run(dir.path(), &env_ja(&dir), &["novel.txt"]);
    assert_eq!(ran.exit(), Exit::Project);
    assert_eq!(
        ran.err,
        format!(
            "novnow: ディレクトリではありません\n  対象: {}\n",
            file.display()
        )
    );
}

#[test]
fn a_v0_project_is_the_example_of_section_14_2_4() {
    let dir = TempDir::new("cli-v0");
    dir.write("hoshikuzu/characters.json", "{\"characters\":[]}\n");
    dir.write("hoshikuzu/project.json", "{\"title\":\"星屑\"}\n");
    let root = dir.join("hoshikuzu");
    let before = tree(dir.path());
    let ran = run(dir.path(), &env_ja(&dir), &["hoshikuzu"]);
    assert_eq!(ran.exit(), Exit::Project);
    let want = spec_block("#### 14.2.4 v0 プロジェクトを指した場合")
        .replace("/home/user/novels/hoshikuzu", &display_path(&root));
    assert_eq!(ran.err, want);
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn a_v0_project_json_and_a_newer_schema_are_exit_3() {
    let dir = TempDir::new("cli-schema");
    let root = minimal_project(&dir, "old");
    std::fs::write(root.join("project.json"), "{\"title\":\"古い\"}\n").expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["old"]);
    assert_eq!(ran.exit(), Exit::Project);
    assert!(ran.err.starts_with(
        "novnow: このプロジェクトは v0 形式です（project.json の schema_version が無いか 0 です）\n"
    ));
    assert!(ran.err.contains(&format!(
        "移行してください: novnow migrate {}\n",
        root.display()
    )));

    let root = minimal_project(&dir, "new");
    std::fs::write(root.join("project.json"), "{\"schema_version\":2}\n").expect("write");
    let ran = run(dir.path(), &env_en(&dir), &["new/"]);
    assert_eq!(ran.exit(), Exit::Project);
    assert_eq!(
        ran.err,
        format!(
            "novnow: this project was made by a newer version of novnow (schema_version 2)\n  target: {}\n\nUpdate novnow to a newer version\n",
            root.display()
        )
    );
}

#[test]
fn a_valid_project_passes_the_precheck_and_asks_for_the_gui_without_writing() {
    let dir = TempDir::new("cli-valid");
    let root = minimal_project(&dir, "novel");
    let before = tree(dir.path());
    let ran = run(dir.path(), &env_ja(&dir), &["novel"]);
    assert_eq!(ran.outcome, Outcome::Gui(Some(root.clone())));
    assert_eq!((ran.out.as_str(), ran.err.as_str()), ("", ""));
    // 起動前の検査は何も書かない（履歴に積むのは GUI が開けたとき、§14.4）
    assert_eq!(tree(dir.path()), before);
    assert!(!dir.join("state").exists());
}

#[test]
fn without_a_gui_the_binary_says_so_and_exits_2() {
    let dir = TempDir::new("cli-nogui");
    let env = env_ja(&dir);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let exit = {
        let mut cli = Cli::new(
            &env,
            &mut out,
            &mut err,
            dir.path().to_path_buf(),
            Terminal::default(),
        );
        cli.gui_unavailable()
    };
    assert_eq!(exit, Exit::Usage);
    assert!(out.is_empty());
    assert_eq!(
        String::from_utf8(err).expect("utf8"),
        "novnow: この novnow には GUI が含まれていません\n使い方: novnow --help\n"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_is_exit_4() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("cli-unreadable");
    let root = minimal_project(&dir, "novel");
    let locked = root.join("contents");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let readable = std::fs::read_dir(&locked).is_ok();
    let ran = run(dir.path(), &env_ja(&dir), &["novel"]);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if readable {
        return; // root で走っている（権限で読めなくできない）
    }
    assert_eq!(ran.exit(), Exit::Io);
    assert!(
        ran.err.starts_with("novnow: 読み込めませんでした: "),
        "{}",
        ran.err
    );
}

// ============================================================ novnow check

#[test]
fn check_of_a_fresh_project_has_no_diagnostics() {
    let dir = TempDir::new("cli-check-fresh");
    let root = dir.mkdir("novel");
    assert_eq!(run(&root, &env_ja(&dir), &["new"]).exit(), Exit::Ok);
    let before = tree(dir.path());
    let ran = run(&root, &env_ja(&dir), &["check", "."]);
    assert_eq!(ran.exit(), Exit::Ok);
    assert_eq!(ran.err, "");
    let documents = template::new_project("novel")
        .iter()
        .filter(|file| is_document(&ProjectPath::new(file.path).expect("path")))
        .count();
    assert_eq!(
        ran.out,
        format!("{documents} 個の文書を検査しました（エラー 0・警告 0・情報 0）\n")
    );
    // check は何も書かない（履歴にも）
    assert_eq!(tree(dir.path()), before);
    assert!(!dir.join("state").exists());
}

#[test]
fn check_prints_one_line_per_diagnostic_and_exits_5_on_errors() {
    let dir = TempDir::new("cli-check-errors");
    let root = minimal_project(&dir, "novel");
    std::fs::write(
        root.join("contents/01-01.txt"),
        "@01-01-01 屋根裏\n「なに」@謎の人\n'未登録の語が出る。\n;\n;\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Diagnostics);
    assert_eq!(Exit::Diagnostics.code(), 5);
    assert_eq!(ran.err, "");
    assert_eq!(
        ran.out,
        "contents/01-01.txt:2:5: E012 登場人物 謎の人 は characters.md にいません\n\
         contents/01-01.txt:3:1: E001 辞書にない参照です: 未登録の語が出る\n\
         contents/01-01.txt:5:1: W012 空の節です\n\
         \n\
         6 個の文書を検査しました（エラー 2・警告 1・情報 0）\n"
    );
    let ran = run(dir.path(), &env_en(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Diagnostics);
    assert!(ran
        .out
        .starts_with("contents/01-01.txt:2:5: E012 Character 謎の人 is not defined\n"));
    assert!(ran
        .out
        .ends_with("\nchecked 6 documents (errors: 2, warnings: 1, info: 0)\n"));
}

#[test]
fn check_with_only_warnings_exits_0() {
    let dir = TempDir::new("cli-check-warnings");
    let root = minimal_project(&dir, "novel");
    std::fs::write(
        root.join("contents/01-01.txt"),
        "@01-01-01\n「こんにちは」\n;\n;\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Ok);
    assert!(ran.out.contains(": W012 "), "{}", ran.out);
    assert!(
        ran.out.ends_with("（エラー 0・警告 1・情報 0）\n"),
        "{}",
        ran.out
    );
}

#[test]
fn check_shows_diagnostics_without_a_position_with_the_path_only() {
    let dir = TempDir::new("cli-check-json");
    let root = minimal_project(&dir, "novel");
    std::fs::write(
        root.join("project.json"),
        "{\"schema_version\": 1, \"lexicon\": {\"word_classes\": {\"a\": \"[\"}}}\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Diagnostics);
    assert!(
        ran.out.starts_with("project.json:1:"),
        "位置のある E027: {}",
        ran.out
    );
    assert!(ran.out.contains(" E027 語クラス a の正規表現が不正です\n"));
}

#[test]
fn check_of_a_broken_project_uses_the_startup_messages() {
    let dir = TempDir::new("cli-check-broken");
    let root = spec_like_incomplete(&dir, "untitled");
    let ran = run(&root, &env_ja(&dir), &["check", "."]);
    assert_eq!(ran.exit(), Exit::Project);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        spec_block("#### 14.2.2 出力（ja）")
            .replace("/home/user/novels/untitled", &display_path(&root))
    );
    let ran = run(dir.path(), &env_ja(&dir), &["check", "nope"]);
    assert_eq!(ran.exit(), Exit::Project);
}

#[test]
fn check_lists_files_that_are_not_utf8_and_exits_4() {
    let dir = TempDir::new("cli-check-utf8");
    let root = minimal_project(&dir, "novel");
    std::fs::write(root.join("contents/01-02.txt"), [0x82, 0xa0, b'\n']).expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Io);
    assert_eq!(Exit::Io.code(), 4);
    assert_eq!(
        ran.err,
        format!(
            "novnow: UTF-8 として読めないファイルがあります\n  対象: {}\n\nUTF-8 として読めないファイル:\n  - contents/01-02.txt\n",
            root.display()
        )
    );
}

#[test]
fn colors_are_used_only_on_a_terminal_without_no_color() {
    let dir = TempDir::new("cli-color");
    let root = minimal_project(&dir, "novel");
    std::fs::write(root.join("contents/01-01.txt"), "@01-01-01\n'未登録\n").expect("write");
    let tty = Terminal {
        stdin: true,
        stdout: true,
        stderr: true,
    };
    let colored = run_on(dir.path(), &env_ja(&dir), tty, &["check", "novel"]);
    assert!(
        colored.out.contains("\u{1b}[1;31mE001\u{1b}[0m"),
        "{}",
        colored.out
    );
    let plain = run_on(
        dir.path(),
        &env_ja(&dir).var("NO_COLOR", "1"),
        tty,
        &["check", "novel"],
    );
    assert!(!plain.out.contains('\u{1b}'), "{}", plain.out);
    let mut windows = env_ja(&dir);
    windows.os = Os::Windows;
    let plain = run_on(dir.path(), &windows, tty, &["check", "novel"]);
    assert!(!plain.out.contains('\u{1b}'));
    // パイプ（端末でない）にも付けない
    let piped = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert!(!piped.out.contains('\u{1b}'));
    // 失敗の見出しは標準エラーが端末のときだけ
    let broken = run_on(dir.path(), &env_ja(&dir), tty, &["nope"]);
    assert!(broken.err.starts_with("\u{1b}[1;31mnovnow:\u{1b}[0m "));
}

// ============================================================ novnow new（§14.5）

#[test]
fn new_creates_the_whole_template_in_an_empty_directory() {
    let dir = TempDir::new("cli-new");
    let root = dir.mkdir("星屑の時計");
    let ran = run(&root, &env_ja(&dir), &["new"]);
    assert_eq!(ran.exit(), Exit::Ok);
    assert_eq!(ran.err, "");
    assert_eq!(
        ran.out,
        format!(
            "novnow: 新しいプロジェクトを作りました\n  対象: {}\n\n検査する: novnow check .\n",
            root.display()
        )
    );
    let files = tree(&root);
    let want: BTreeMap<String, Vec<u8>> = template::new_project("星屑の時計")
        .into_iter()
        .map(|file| (file.path.to_owned(), file.text.into_bytes()))
        .collect();
    assert_eq!(files, want);
    let project: serde_json::Value =
        serde_json::from_slice(&files["project.json"]).expect("project.json");
    assert_eq!(project["title"], "星屑の時計");
    // 原子的保存の一時ファイルも履歴も残らない
    assert!(!dir.join("state").exists());
}

#[test]
fn new_creates_a_missing_directory_and_names_it_in_the_hint() {
    let dir = TempDir::new("cli-new-missing");
    let ran = run(dir.path(), &env_ja(&dir), &["new", "novels/hoshi"]);
    assert_eq!(ran.exit(), Exit::Ok);
    let root = dir.join("novels/hoshi");
    assert!(root.join("project.json").is_file());
    assert!(ran
        .out
        .ends_with(&format!("\n検査する: novnow check {}\n", root.display())));
    let project: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("project.json")).expect("read"))
            .expect("json");
    assert_eq!(project["title"], "hoshi");
    // そのまま check を通る
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["check", "novels/hoshi"]).exit(),
        Exit::Ok
    );
}

#[test]
fn new_never_overwrites_and_lists_what_it_created_and_kept() {
    let dir = TempDir::new("cli-new-keep");
    let root = minimal_project(&dir, "novel");
    let before = tree(&root);
    let ran = run(&root, &env_ja(&dir), &["new"]);
    assert_eq!(ran.exit(), Exit::Ok);
    // 既存のファイルは 1 バイトも変わらない（MUST）
    let after = tree(&root);
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{path} が変わった");
    }
    let kept = [
        "project.json",
        "characters.md",
        "words.md",
        "contents/01-01.txt",
        "plots/all.txt",
        "materials/index.json",
    ];
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(
        lines[0],
        "novnow: 足りないファイルを作りました（既存のファイルは上書きしていません）"
    );
    let created_at = lines
        .iter()
        .position(|line| *line == "作成したファイル:")
        .expect("作成");
    let kept_at = lines
        .iter()
        .position(|line| *line == "上書きしなかったファイル:")
        .expect("上書きしなかった");
    let listed = |from: usize| -> Vec<String> {
        lines[from + 1..]
            .iter()
            .take_while(|line| line.starts_with("  - "))
            .map(|line| line["  - ".len()..].to_owned())
            .collect()
    };
    let template_order: Vec<&str> = template::new_project("novel")
        .iter()
        .map(|file| file.path)
        .collect();
    let want_kept: Vec<String> = template_order
        .iter()
        .filter(|path| kept.contains(path))
        .map(|path| path.to_string())
        .collect();
    let want_created: Vec<String> = template_order
        .iter()
        .filter(|path| !kept.contains(path))
        .map(|path| path.to_string())
        .collect();
    assert_eq!(listed(kept_at), want_kept);
    assert_eq!(listed(created_at), want_created);
    for path in &want_created {
        assert!(root.join(path).is_file(), "{path}");
    }
}

#[test]
fn new_on_a_complete_project_creates_nothing() {
    let dir = TempDir::new("cli-new-twice");
    let root = dir.mkdir("novel");
    assert_eq!(run(&root, &env_ja(&dir), &["new"]).exit(), Exit::Ok);
    let before = tree(&root);
    let ran = run(&root, &env_en(&dir), &["new", "."]);
    assert_eq!(ran.exit(), Exit::Ok);
    assert!(ran
        .out
        .starts_with("novnow: nothing to create (every file already exists)\n"));
    assert!(!ran.out.contains("Created files:"));
    assert!(ran
        .out
        .contains("Kept files (not overwritten):\n  - project.json\n"));
    assert_eq!(tree(&root), before);
}

#[test]
fn new_refuses_a_v0_project_and_a_newer_one_without_writing() {
    let dir = TempDir::new("cli-new-v0");
    dir.write("old/characters.json", "{}\n");
    let before = tree(dir.path());
    let ran = run(dir.path(), &env_ja(&dir), &["new", "old"]);
    assert_eq!(ran.exit(), Exit::Project);
    assert!(ran
        .err
        .starts_with("novnow: このプロジェクトは v0 形式です（characters.json を検出）\n"));
    assert_eq!(tree(dir.path()), before);

    let root = minimal_project(&dir, "newer");
    std::fs::write(root.join("project.json"), "{\"schema_version\":3}\n").expect("write");
    let before = tree(dir.path());
    let ran = run(dir.path(), &env_ja(&dir), &["new", "newer"]);
    assert_eq!(ran.exit(), Exit::Project);
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn new_on_a_file_is_exit_4() {
    let dir = TempDir::new("cli-new-file");
    dir.write("novel", "ファイル\n");
    let ran = run(dir.path(), &env_ja(&dir), &["new", "novel"]);
    assert_eq!(ran.exit(), Exit::Io);
    assert!(ran.err.starts_with("novnow: ディレクトリではありません\n"));
}

#[test]
fn new_writes_nothing_outside_the_project() {
    let dir = TempDir::new("cli-new-outside");
    dir.write("outside/keep.txt", "そのまま\n");
    let before = tree(&dir.join("outside"));
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["new", "novel"]).exit(),
        Exit::Ok
    );
    assert_eq!(tree(&dir.join("outside")), before);
    let top: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read_dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let mut top = top;
    top.sort();
    assert_eq!(top, ["novel", "outside"]);
}

// ============================================================ 枠だけのサブコマンド

#[test]
fn export_mcp_and_migrate_are_frames_only_in_m6() {
    let dir = TempDir::new("cli-frames");
    minimal_project(&dir, "novel");
    for args in [
        &["export", "novel"][..],
        &["export", "novel", "--range", "@01"],
        &["mcp", "novel"],
    ] {
        let ran = run(dir.path(), &env_ja(&dir), args);
        assert_eq!(ran.exit(), Exit::Usage, "{args:?}");
        assert_eq!(
            ran.err,
            format!(
                "novnow: novnow {} はまだ実装されていません\n使い方: novnow --help\n",
                args[0]
            )
        );
    }
    // export / mcp は先に §14.2 の検査をする
    spec_like_incomplete(&dir, "broken");
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["export", "broken"]).exit(),
        Exit::Project
    );
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["mcp", "broken"]).exit(),
        Exit::Project
    );
    // migrate の対象は v0（§14.2 の検査に落ちる側）なので、開けるかだけを見る
    dir.write("old/characters.json", "{}\n");
    let ran = run(dir.path(), &env_ja(&dir), &["migrate", "old"]);
    assert_eq!(ran.exit(), Exit::Usage);
    assert!(ran
        .err
        .starts_with("novnow: novnow migrate はまだ実装されていません\n"));
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["migrate", "nope"]).exit(),
        Exit::Project
    );
}

// ============================================================ 使い方・版・引数の誤り

#[test]
fn help_goes_to_stdout_and_explains_how_to_open_a_directory_named_new() {
    let dir = TempDir::new("cli-help");
    for (env, needle) in [
        (
            env_ja(&dir),
            "new という名前のディレクトリを開くときは novnow ./new と書いてください。",
        ),
        (
            env_en(&dir),
            "To open a directory named new, write novnow ./new.",
        ),
    ] {
        let ran = run(dir.path(), &env, &["--help"]);
        assert_eq!(ran.exit(), Exit::Ok);
        assert_eq!(ran.err, "");
        assert!(ran.out.starts_with(&format!("novnow {} - ", cli::VERSION)));
        assert!(ran.out.contains(needle), "{}", ran.out);
        assert!(ran.out.contains(PROJECT_STRUCTURE_DOC_URL));
        for synopsis in [
            "novnow <dir>",
            "novnow new [dir]",
            "novnow check <dir>",
            "novnow export <dir> [options]",
            "novnow migrate <dir>",
            "novnow mcp <dir>",
            "novnow --gui",
            "novnow --help | -h",
            "novnow --version | -V",
        ] {
            assert!(ran.out.contains(&format!("  {synopsis} ")), "{synopsis}");
        }
    }
}

#[test]
fn a_terminal_without_arguments_gets_the_usage_on_stderr_and_exit_2() {
    let dir = TempDir::new("cli-usage");
    let tty = Terminal {
        stdin: true,
        stdout: false,
        stderr: false,
    };
    let ran = run_on(dir.path(), &env_ja(&dir), tty, &[]);
    assert_eq!(ran.exit(), Exit::Usage);
    assert_eq!(Exit::Usage.code(), 2);
    assert_eq!(ran.out, "");
    let help = run(dir.path(), &env_ja(&dir), &["--help"]);
    assert_eq!(ran.err, help.out);
    // 端末でなければ GUI（M6 では main.rs が「GUI が含まれていない」にする）
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &[]).outcome,
        Outcome::Gui(None)
    );
}

#[test]
fn the_version_is_printed_on_stdout() {
    let dir = TempDir::new("cli-version");
    let ran = run(dir.path(), &env_ja(&dir), &["-V"]);
    assert_eq!(ran.exit(), Exit::Ok);
    assert_eq!(ran.out, format!("novnow {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn argument_errors_are_exit_2_with_a_pointer_to_the_help() {
    let dir = TempDir::new("cli-args");
    let cases: [(&[&str], &str); 4] = [
        (&["-x"], "不明なオプションです: -x"),
        (&["a", "b"], "余分な引数です: b"),
        (
            &["check"],
            "novnow check にはディレクトリを指定してください",
        ),
        (&["new", "--force"], "不明なオプションです: --force"),
    ];
    for (args, message) in cases {
        let ran = run(dir.path(), &env_ja(&dir), args);
        assert_eq!(ran.exit(), Exit::Usage, "{args:?}");
        assert_eq!(ran.out, "");
        assert_eq!(
            ran.err,
            format!("novnow: {message}\n使い方: novnow --help\n")
        );
    }
}

// ============================================================ パスと定数

#[test]
fn paths_are_shown_plainly_and_quoted_only_when_needed() {
    assert_eq!(
        display_path(Path::new(r"\\?\C:\novels\hoshi")),
        r"C:\novels\hoshi"
    );
    assert_eq!(
        display_path(Path::new(r"\\?\UNC\server\share\x")),
        r"\\server\share\x"
    );
    assert_eq!(display_path(Path::new("/home/u/星屑")), "/home/u/星屑");
    assert_eq!(
        shell_quote("/home/u/星屑の時計", Os::Linux),
        "/home/u/星屑の時計"
    );
    assert_eq!(
        shell_quote("/home/u/my novel", Os::Linux),
        "'/home/u/my novel'"
    );
    assert_eq!(shell_quote("/home/u/it's", Os::MacOs), r"'/home/u/it'\''s'");
    assert_eq!(
        shell_quote(r"C:\novels\hoshi", Os::Windows),
        r"C:\novels\hoshi"
    );
    assert_eq!(
        shell_quote(r"C:\my novels", Os::Windows),
        r#""C:\my novels""#
    );
    assert_eq!(shell_quote("", Os::Linux), "''");
}

#[test]
fn the_structure_doc_url_is_written_once_and_matches_section_14_2_1() {
    // §14.2.1「これらの URL はハードコードせず、この定数群のみを参照する（MUST）」
    let block = spec_block("#### 14.2.1 定数");
    assert!(
        block.contains(&format!("\"{PROJECT_STRUCTURE_DOC_URL}\"")),
        "§14.2.1 の値と違う"
    );
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(Path::new("src"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for _ in text.matches("sonneko.github.io/novnow/guide") {
            found.push(file.display().to_string());
        }
    }
    assert_eq!(found, ["src/core/model/project.rs"]);
}

// ============================================================ 実行ファイル（子プロセス）

/// 環境変数を空にして実行ファイルを走らせる（履歴の置き場所と言語だけを渡す）。
fn binary(dir: &TempDir, cwd: &Path, args: &[&str]) -> std::process::Output {
    Process::new(env!("CARGO_BIN_EXE_novnow"))
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("HOME", dir.join("home"))
        .env("NOVNOW_RECENT_FILE", dir.join("state/recent.json"))
        .env("NOVNOW_LANG", "ja")
        .stdin(Stdio::null())
        .output()
        .expect("novnow を起動できない")
}

#[test]
fn binary_an_empty_directory_exits_3_and_starts_no_gui() {
    let dir = TempDir::new("bin-empty");
    let root = dir.mkdir("novel");
    let before = tree(dir.path());
    let output = binary(&dir, &root, &["."]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let err = String::from_utf8(output.stderr).expect("utf8");
    assert!(err.starts_with("novnow: このディレクトリは novnow プロジェクトではありません\n"));
    assert!(err.ends_with(&format!(
        "プロジェクト構造: {PROJECT_STRUCTURE_DOC_URL}\nこのディレクトリに新規作成する場合: novnow new\n"
    )));
    assert_eq!(tree(dir.path()), before);
}

#[test]
fn binary_new_then_check_then_open() {
    let dir = TempDir::new("bin-new");
    let root = dir.mkdir("novel");
    let output = binary(&dir, &root, &["new"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(root.join("prompts/writer.md").is_file());
    let output = binary(&dir, &root, &["check", "."]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let out = String::from_utf8(output.stdout).expect("utf8");
    assert!(out.ends_with("（エラー 0・警告 0・情報 0）\n"), "{out}");
    // 検査は通るが、M6 の実行ファイルは GUI を含まないので 2
    let output = binary(&dir, &root, &["."]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("utf8"),
        "novnow: この novnow には GUI が含まれていません\n使い方: novnow --help\n"
    );
    // ここまで履歴にも HOME にも書いていない
    assert!(!dir.join("state").exists());
    assert!(!dir.join("home").exists());
}

#[test]
fn binary_without_arguments_and_without_a_terminal_wants_the_gui() {
    let dir = TempDir::new("bin-noargs");
    let output = binary(&dir, dir.path(), &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)
        .expect("utf8")
        .starts_with("novnow: この novnow には GUI が含まれていません\n"));
}

#[test]
fn binary_help_version_and_check_errors() {
    let dir = TempDir::new("bin-help");
    let output = binary(&dir, dir.path(), &["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8(output.stdout)
        .expect("utf8")
        .contains("novnow ./new"));
    let output = binary(&dir, dir.path(), &["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf8"),
        format!("novnow {}\n", env!("CARGO_PKG_VERSION"))
    );
    let output = binary(&dir, dir.path(), &["--frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    let root = minimal_project(&dir, "novel");
    std::fs::write(root.join("contents/01-01.txt"), "@01-01-01\n'未登録\n").expect("write");
    let output = binary(&dir, dir.path(), &["check", "novel"]);
    assert_eq!(output.status.code(), Some(5));
    // 言語の上書きが無ければ OS のロケール（LANG=C → en）
    let output = Process::new(env!("CARGO_BIN_EXE_novnow"))
        .args(["check", "novel"])
        .current_dir(dir.path())
        .env_clear()
        .env("NOVNOW_RECENT_FILE", dir.join("state/recent.json"))
        .env("LANG", "C")
        .output()
        .expect("run");
    assert!(String::from_utf8(output.stdout)
        .expect("utf8")
        .contains(" E001 Unknown reference: 未登録\n"));
}
