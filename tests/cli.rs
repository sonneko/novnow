//! CLI（§14、`docs/history.md` のM6）。起動の判定（§14.3）・起動前の検査と表示（§14.2）・`new`（§14.5）・
//! `check`・終了コード（§14.2.5）。
//!
//! 大半は `novnow::cli::run` をプロセス内で走らせる（出力はバッファ、環境は偽の `Env`、カレント
//! ディレクトリは引数で渡す。**プロセスの環境変数もカレントディレクトリも書き換えない**）。
//! 末尾の `binary_*` は実際の実行ファイル（`CARGO_BIN_EXE_novnow`）を、環境変数を空にした子プロセス
//! として走らせる（終了コードと標準出力・標準エラーの分かれ方を本物で確かめる）。
//!
//! §14.2.2 / §14.2.3 の出力例は**仕様書を読み直して一字一句**突き合わせる。

mod novel;
mod sandbox;
mod world;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};

use novnow::adapters::project_load::is_document;
use novnow::cli::{
    self, decide, display_path, shell_quote, ArgError, Cli, Command, Exit, Launch, Outcome,
    Terminal,
};
use novnow::core::agents::server;
use novnow::core::model::project::PROJECT_STRUCTURE_DOC_URL;
use novnow::core::policy::ProjectPath;
use novnow::core::template;
use novnow::ports::env::Os;

use sandbox::{minimal_project, tree, FakeEnv, TempDir};

const SPEC: &str = include_str!("../docs/specifications/platform.md");

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

/// §14.2.2 の例と同じ不足（project.toml / characters.md / plots/all.cnov）を持つディレクトリ。
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
    // `docs/history.md` の完了条件「空ディレクトリで novnow . → エラー ＋ exit 3。GUI を出さない」
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
            "novnow: このディレクトリは novnow プロジェクトではありません\n  対象: {}\n\n不足しているもの:\n  - project.toml\n  - characters.md\n  - words.md\n  - materials/index.json\n  - plots/all.cnov\n  - contents/\n  - plots/\n\nプロジェクト構造: {PROJECT_STRUCTURE_DOC_URL}\nこのディレクトリに新規作成する場合: novnow new\n",
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
fn a_newer_schema_is_exit_3() {
    let dir = TempDir::new("cli-schema");
    let root = minimal_project(&dir, "new");
    std::fs::write(root.join("project.toml"), "schema_version = 3\n").expect("write");
    let ran = run(dir.path(), &env_en(&dir), &["new/"]);
    assert_eq!(ran.exit(), Exit::Project);
    assert_eq!(
        ran.err,
        format!(
            "novnow: this project was made by a newer version of novnow (schema_version 3)\n  target: {}\n\nUpdate novnow to a newer version\n",
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
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
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
        root.join("contents/01-01.cnov"),
        "@01-01-01 屋根裏\n「なに」@謎の人\n'未登録の語が出る。\n;\n;\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Diagnostics);
    assert_eq!(Exit::Diagnostics.code(), 5);
    assert_eq!(ran.err, "");
    assert_eq!(
        ran.out,
        "contents/01-01.cnov:2:5: [本文 @01-01-01-01] E012 登場人物 謎の人は characters.md にいません\n\
         contents/01-01.cnov:3:1: [本文 @01-01-01-01] E001 辞書にない参照です: 未登録\n\
         contents/01-01.cnov:5:1: [本文 @01-01-01-02] W012 空の節です\n\
         \n\
         6 個の文書を検査しました（エラー 2・警告 1・情報 0）\n"
    );
    let ran = run(dir.path(), &env_en(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Diagnostics);
    assert!(ran.out.starts_with(
        "contents/01-01.cnov:2:5: [prose @01-01-01-01] E012 Character 謎の人 is not defined\n"
    ));
    assert!(ran
        .out
        .ends_with("\nchecked 6 documents (errors: 2, warnings: 1, info: 0)\n"));
}

#[test]
fn check_with_only_warnings_exits_0() {
    let dir = TempDir::new("cli-check-warnings");
    let root = minimal_project(&dir, "novel");
    std::fs::write(
        root.join("contents/01-01.cnov"),
        "@01-01-01\n「こんにちは」\n;\n;\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Ok);
    assert!(
        ran.out
            .contains("contents/01-01.cnov:4:1: [本文 @01-01-01-02] W012 空の節です\n"),
        "{}",
        ran.out
    );
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
        root.join("project.toml"),
        "schema_version = 2\n\n[lexicon.word_classes]\n\"a\" = \"[\"\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Diagnostics);
    assert!(
        ran.out.starts_with("project.toml:4:7:"),
        "位置のある E027: {}",
        ran.out
    );
    assert!(ran
        .out
        .contains(" [作品の設定] E027 語クラス aの正規表現が不正です\n"));
}

#[test]
fn check_names_the_place_in_the_writers_terms() {
    // §14.2.5（第 2.11 版）: `パス:行:列:` に加えて、作家の言葉での「どこ」
    let dir = TempDir::new("cli-check-place");
    let root = minimal_project(&dir, "novel");
    std::fs::write(
        root.join("characters.md"),
        "# 星野ひかり\nrole: 主人公\n\n## modes\n\n### A ふだん\nrange: @99 .. @01\n",
    )
    .expect("write");
    std::fs::write(
        root.join("plots/01-01.cnov"),
        "@01-01-01 屋根裏\n'未登録の語を探す\n;\n",
    )
    .expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert!(
        ran.out.contains(
            "characters.md:7:8: [登場人物「星野ひかり」のモード A ふだん] E016 範囲の指定が逆です\n"
        ),
        "{}",
        ran.out
    );
    assert!(
        ran.out
            .contains("plots/01-01.cnov:2:1: [話プロット @01-01-01-01] E001 "),
        "{}",
        ran.out
    );
    let ran = run(dir.path(), &env_en(&dir), &["check", "novel"]);
    assert!(
        ran.out
            .contains("characters.md:7:8: [character \"星野ひかり\", mode A ふだん] E016 "),
        "{}",
        ran.out
    );
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
    std::fs::write(root.join("contents/01-02.cnov"), [0x82, 0xa0, b'\n']).expect("write");
    let ran = run(dir.path(), &env_ja(&dir), &["check", "novel"]);
    assert_eq!(ran.exit(), Exit::Io);
    assert_eq!(Exit::Io.code(), 4);
    assert_eq!(
        ran.err,
        format!(
            "novnow: UTF-8 として読めないファイルがあります\n  対象: {}\n\nUTF-8 として読めないファイル:\n  - contents/01-02.cnov\n",
            root.display()
        )
    );
}

#[test]
fn colors_are_used_only_on_a_terminal_without_no_color() {
    let dir = TempDir::new("cli-color");
    let root = minimal_project(&dir, "novel");
    std::fs::write(root.join("contents/01-01.cnov"), "@01-01-01\n'未登録\n").expect("write");
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
    // GUI を含むビルド（`ui` feature）は「開く」、含まないビルドは「検査する」を案内する
    let next = if novnow::cli::GUI_AVAILABLE {
        "開く: novnow ."
    } else {
        "検査する: novnow check ."
    };
    assert_eq!(
        ran.out,
        format!(
            "novnow: 新しいプロジェクトを作りました\n  対象: {}\n\n{next}\n",
            root.display()
        )
    );
    let files = tree(&root);
    let want: BTreeMap<String, Vec<u8>> = template::new_project("星屑の時計")
        .into_iter()
        .map(|file| (file.path.to_owned(), file.text.into_bytes()))
        .collect();
    assert_eq!(files, want);
    let project = novnow::core::syntax::project_toml::settings(
        std::str::from_utf8(&files["project.toml"]).expect("utf-8"),
    );
    assert_eq!(project.title, "星屑の時計");
    assert_eq!(project.schema_version, 2);
    // 原子的保存の一時ファイルも履歴も残らない
    assert!(!dir.join("state").exists());
}

#[test]
fn new_creates_a_missing_directory_and_names_it_in_the_hint() {
    let dir = TempDir::new("cli-new-missing");
    let ran = run(dir.path(), &env_ja(&dir), &["new", "novels/hoshi"]);
    assert_eq!(ran.exit(), Exit::Ok);
    let root = dir.join("novels/hoshi");
    assert!(root.join("project.toml").is_file());
    let next = if novnow::cli::GUI_AVAILABLE {
        format!("\n開く: novnow {}\n", root.display())
    } else {
        format!("\n検査する: novnow check {}\n", root.display())
    };
    assert!(ran.out.ends_with(&next), "{}", ran.out);
    let project = novnow::core::syntax::project_toml::settings(
        &std::fs::read_to_string(root.join("project.toml")).expect("read"),
    );
    assert_eq!(project.title, "hoshi");
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
        "project.toml",
        "characters.md",
        "words.md",
        "contents/01-01.cnov",
        "plots/all.cnov",
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
    let template_order: Vec<String> = template::new_project("novel")
        .iter()
        .map(|file| file.path.clone())
        .collect();
    let want_kept: Vec<String> = template_order
        .iter()
        .filter(|path| kept.contains(&path.as_str()))
        .map(|path| path.to_string())
        .collect();
    let want_created: Vec<String> = template_order
        .iter()
        .filter(|path| !kept.contains(&path.as_str()))
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
        .contains("Kept files (not overwritten):\n  - project.toml\n"));
    assert_eq!(tree(&root), before);
}

#[test]
fn new_refuses_a_newer_project_without_writing() {
    let dir = TempDir::new("cli-new-too-new");
    let root = minimal_project(&dir, "newer");
    std::fs::write(root.join("project.toml"), "schema_version = 3\n").expect("write");
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

// ============================================================ novnow export（§17.2、M9）

/// 実プロジェクト（`tests/novel`）をディレクトリに書く。
fn write_novel(dir: &TempDir, root: &str) -> PathBuf {
    for (path, text) in novel::files() {
        dir.write(&format!("{root}/{path}"), text);
    }
    dir.join(root)
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn export_to_stdout_prints_only_the_text() {
    let dir = TempDir::new("cli-export-stdout");
    let root = write_novel(&dir, "novel");
    let before = tree(&root);
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &["export", "novel", "--out", "-"],
    );
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    assert_eq!(ran.out, novel::EXPECTED);
    assert_eq!(ran.err, "");
    assert_eq!(tree(&root), before, "標準出力のときは何も書かない");
    // `--name=value` の形・範囲（`@` なし）・状態
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &[
            "export",
            "novel",
            "--range=01-02 .. 02",
            "--state",
            "fixed",
            "--out=-",
        ],
    );
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    assert_eq!(
        ran.out,
        "旧校舎の扉は、錆びついていた。\r\n\r\n扉の向こうは暗かった。\\ 落書きの「「」が見える。\r\n"
    );
    // 同じオプションが 2 回なら後のほう
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &[
            "export", "novel", "--range", "@01", "--range", "@02", "--out", "-",
        ],
    );
    assert_eq!(ran.out, "補習の教室で、アルス は居眠りをしていた。\r\n");
}

#[test]
fn export_writes_the_default_file_inside_the_project() {
    let dir = TempDir::new("cli-export-default");
    let root = minimal_project(&dir, "novel");
    let ran = run(&root, &env_ja(&dir), &["export", "."]);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    // exports/<title>-<範囲>-<YYYYMMDD-HHMM>.txt（§17.2）
    let names = names_in(&root.join("exports"));
    assert_eq!(names.len(), 1, "{names:?}");
    let stamp = names[0]
        .strip_prefix("星屑の時計-all-")
        .and_then(|rest| rest.strip_suffix(".txt"))
        .unwrap_or_else(|| panic!("{names:?}"));
    assert!(
        stamp.len() == 13
            && stamp.as_bytes()[8] == b'-'
            && stamp.bytes().filter(u8::is_ascii_digit).count() == 12,
        "{stamp}"
    );
    let target = root.join("exports").join(&names[0]);
    assert_eq!(
        std::fs::read_to_string(&target).expect("書き出したファイル"),
        "「こんにちは」\r\n"
    );
    assert_eq!(
        ran.out,
        format!(
            "novnow: 書き出しました（1 節・7 字）\n  書き出し先: {}\n",
            display_path(&target)
        )
    );
    assert_eq!(ran.err, "");
    // 範囲はファイル名に入る（英語）
    let ran = run(&root, &env_en(&dir), &["export", ".", "--range", "@01-01"]);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    assert!(
        ran.out
            .starts_with("novnow: exported 1 sections (7 characters)\n  output: "),
        "{}",
        ran.out
    );
    assert!(names_in(&root.join("exports"))
        .iter()
        .any(|name| name.starts_with("星屑の時計-01-01-")));
    // 履歴には書かない
    assert!(!dir.join("state").exists());
}

#[test]
fn export_writes_only_the_chosen_file_outside_the_project() {
    let dir = TempDir::new("cli-export-out");
    let root = minimal_project(&dir, "novel");
    dir.mkdir("outside");
    let before = tree(&root);
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &["export", "novel", "--out", "outside/星屑.md"],
    );
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let written = dir.join("outside/星屑.md");
    assert_eq!(
        std::fs::read_to_string(&written).expect("書き出したファイル"),
        "「こんにちは」\r\n"
    );
    // 指定した 1 個だけ（一時ファイルも残さない）・プロジェクトには何も書かない・履歴にも書かない
    assert_eq!(names_in(&dir.join("outside")), ["星屑.md"]);
    assert_eq!(tree(&root), before);
    assert!(!dir.join("state").exists());
    assert!(ran
        .out
        .ends_with(&format!("  書き出し先: {}\n", display_path(&written))));
    // 親ディレクトリは作らない（4）
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &["export", "novel", "--out", "nowhere/a.txt"],
    );
    assert_eq!(ran.exit(), Exit::Io);
    assert!(!dir.join("nowhere").exists());
    assert!(
        ran.err.starts_with("novnow: 書き込めませんでした: "),
        "{}",
        ran.err
    );
    // 名前が .txt / .md でなければ E060 で書かない（4）
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &["export", "novel", "--out", "outside/a.zip"],
    );
    assert_eq!(ran.exit(), Exit::Io);
    assert!(
        ran.err
            .starts_with("novnow: 書き込めませんでした: プロジェクトの外には書き込めません: "),
        "{}",
        ran.err
    );
    assert_eq!(names_in(&dir.join("outside")), ["星屑.md"]);
    // プロジェクトの中を指定してもよい（ふつうの保存）
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &["export", "novel", "--out", "novel/exports/公開用.txt"],
    );
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    assert!(root.join("exports/公開用.txt").is_file());
}

#[test]
fn export_option_errors_are_exit_2_and_empty_selections_write_nothing() {
    let dir = TempDir::new("cli-export-args");
    minimal_project(&dir, "novel");
    let cases: [(&[&str], &str); 6] = [
        (
            &["export", "novel", "--range"],
            "--rangeに値を指定してください",
        ),
        (
            &["export", "novel", "--range", "--state", "fixed"],
            "--rangeに値を指定してください",
        ),
        (
            &["export", "novel", "--range", "@03 .. @01"],
            "範囲として読めません: @03 .. @01（例: @01-04、@01 .. @03, @05）",
        ),
        (
            &["export", "novel", "--state", "done"],
            "状態は draft / review / fixed のどれかです: done",
        ),
        (
            &["export", "novel", "--out="],
            "--outに値を指定してください",
        ),
        (&["export", "novel", "extra"], "余分な引数です: extra"),
    ];
    for (args, message) in cases {
        let ran = run(dir.path(), &env_ja(&dir), args);
        assert_eq!(ran.exit(), Exit::Usage, "{args:?}");
        assert_eq!(ran.out, "", "{args:?}");
        assert_eq!(
            ran.err,
            format!("novnow: {message}\n使い方: novnow --help\n"),
            "{args:?}"
        );
    }
    // §14.2 の検査（不備は 3）
    spec_like_incomplete(&dir, "broken");
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["export", "broken"]).exit(),
        Exit::Project
    );
    // 当てはまる節が無ければ何も書かずに 0（state の無い節は draft）
    let ran = run(
        dir.path(),
        &env_ja(&dir),
        &["export", "novel", "--state", "fixed"],
    );
    assert_eq!(ran.exit(), Exit::Ok);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        "novnow: 範囲に当てはまる節がありません（何も書き出していません）\n"
    );
    assert!(!dir.join("novel/exports").exists());
    // 使い方にオプションが載っている
    let help = run(dir.path(), &env_ja(&dir), &["--help"]);
    assert!(
        help.out.contains("--state draft|review|fixed"),
        "{}",
        help.out
    );
}

// ============================================================ novnow mcp

#[test]
fn mcp_checks_the_project_first() {
    let dir = TempDir::new("cli-frames");
    // mcp は先に §14.2 の検査をする
    spec_like_incomplete(&dir, "broken");
    assert_eq!(
        run(dir.path(), &env_ja(&dir), &["mcp", "broken"]).exit(),
        Exit::Project
    );
    assert_eq!(
        run_mcp(dir.path(), &env_ja(&dir), "nope", "").exit(),
        Exit::Project
    );
}

/// `novnow mcp` を、標準入力の代わりに `input` を渡して走らせる。
fn run_mcp(cwd: &Path, env: &FakeEnv, dir: &str, input: &str) -> Ran {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut reader = std::io::Cursor::new(input.as_bytes().to_vec());
    let outcome = {
        let mut cli = Cli::new(
            env,
            &mut out,
            &mut err,
            cwd.to_path_buf(),
            Terminal::default(),
        );
        cli.set_input(&mut reader);
        cli::run(&os_args(&["mcp", dir]), &mut cli)
    };
    Ran {
        outcome,
        out: String::from_utf8(out).expect("stdout は UTF-8"),
        err: String::from_utf8(err).expect("stderr は UTF-8"),
    }
}

fn mcp_lines(requests: &[serde_json::Value]) -> String {
    let mut out: String = requests
        .iter()
        .map(|request| request.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

#[test]
fn mcp_serves_the_tools_over_json_lines_and_writes_only_inside_the_project() {
    use serde_json::{json, Value};
    let dir = TempDir::new("cli-mcp");
    let root = minimal_project(&dir, "novel");
    let input = mcp_lines(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "novnow.project.info", "arguments": {}}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "novnow.section.write", "arguments": {"scene": "@01-01-01-01", "text": "x"}}}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "novnow.section.propose", "arguments": {"scene": "@01-01-01-01", "text": "「やあ」"}}}),
        json!({"jsonrpc": "2.0", "id": 6, "method": "resources/list"}),
        json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}),
    ]);
    let ran = run_mcp(dir.path(), &env_ja(&dir), "novel", &input);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let lines: Vec<Value> = ran
        .out
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行 1 メッセージの JSON"))
        .collect();
    assert_eq!(lines.len(), 7, "{}", ran.out);
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "novnow");
    assert_eq!(lines[0]["result"]["protocolVersion"], "2024-11-05");
    // 既存 14 ツールは凍結（§19.8.3）。後ろにカタログ（§19.5.1）が続く。
    // 比較相手は `REF:tests/golden/mcp_tools.json`（実装そのものと比べると退行を検出できない。
    // 内訳の機械検査は `REF:tests/mcp_frozen.rs`）
    let tools = lines[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 87, "ツール表の全件数（凍結 14 + 正規操作 73）");
    let golden: Value = serde_json::from_str(include_str!("golden/mcp_tools.json")).unwrap();
    for (found, frozen) in tools.iter().zip(golden.as_array().unwrap()) {
        assert_eq!(
            found.to_string(),
            frozen.to_string(),
            "既存ツールの定義が変わった（§19.8.3。意図した変更なら              `cargo run --locked --example mcp_tools > tests/golden/mcp_tools.json`）"
        );
    }
    assert_eq!(lines[2]["result"]["isError"], false);
    let info: Value =
        serde_json::from_str(lines[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(info["title"], "星屑の時計");
    assert_eq!(info["sections"], 1);
    // role が無い直接書き込みは E036（文言はカタログ）
    assert_eq!(lines[3]["result"]["isError"], true);
    assert_eq!(
        lines[3]["result"]["content"][0]["text"],
        "この役割の委譲レベルでは書き込めません: -"
    );
    assert_eq!(lines[4]["result"]["isError"], false);
    assert!(root.join("agents/proposals/P0001.md").exists());
    let proposal = std::fs::read_to_string(root.join("agents/proposals/P0001.md")).unwrap();
    assert!(proposal.contains("origin: mcp\n"), "{proposal}");
    assert!(
        proposal.contains("## before\n「こんにちは」\n"),
        "{proposal}"
    );
    // 本文は変わらない
    assert_eq!(
        std::fs::read_to_string(root.join("contents/01-01.cnov")).unwrap(),
        "@01-01-01\n「こんにちは」\n"
    );
    assert_eq!(lines[5]["error"]["code"], -32601);
    assert_eq!(lines[6]["result"], json!({}));
    // 起動時に既定のプロンプトを書き出した（prompts/ が無かったので I050）
    assert!(root.join("prompts/writer.md").exists());
    assert!(root.join("prompts/_shared/style.md").exists());
    assert!(
        ran.err.contains("prompts/ に既定プロンプトを作成しました"),
        "{}",
        ran.err
    );
    // 二度目は何も書かず、通知もしない
    let again = run_mcp(dir.path(), &env_ja(&dir), "novel", "");
    assert_eq!(again.exit(), Exit::Ok);
    assert_eq!(again.err, "");
}

#[cfg(unix)]
#[test]
fn mcp_leaves_prompts_that_are_links_to_outside_alone() {
    // 一覧に無くてもディスクにあるもの（外を指すリンク）には既定を書かない（B-155。書けば規則 10 の違反）
    let dir = TempDir::new("cli-mcp-prompt-link");
    let root = minimal_project(&dir, "novel");
    dir.write("outside/writer.md", "外のプロンプト\n");
    std::fs::create_dir_all(root.join("prompts")).expect("mkdir");
    std::os::unix::fs::symlink(
        dir.join("outside/writer.md"),
        root.join("prompts/writer.md"),
    )
    .expect("ln");
    let ran = run_mcp(dir.path(), &env_ja(&dir), "novel", "");
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    assert_eq!(
        std::fs::read_to_string(dir.join("outside/writer.md")).expect("read"),
        "外のプロンプト\n"
    );
    assert!(root
        .join("prompts/writer.md")
        .symlink_metadata()
        .expect("link")
        .file_type()
        .is_symlink());
    assert!(
        root.join("prompts/plotter.md").is_file(),
        "ほかの欠けたものは書く"
    );
}

#[test]
fn mcp_direct_writes_follow_the_role_level_and_leave_the_audit_trail() {
    use serde_json::{json, Value};
    let dir = TempDir::new("cli-mcp-write");
    let root = minimal_project(&dir, "novel");
    dir.write(
        "novel/project.toml",
        "schema_version = 2\ntitle = \"星屑の時計\"\n\n[agents.roles]\nwriter = \"apply\"\n",
    );
    let input = mcp_lines(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "novnow.section.write", "arguments": {"role": "writer", "scene": "@01-01-01-01", "text": "「さようなら」"}}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "novnow.section.write", "arguments": {"role": "stylist", "scene": "@01-01-01-01", "text": "x"}}}),
    ]);
    let ran = run_mcp(dir.path(), &env_en(&dir), "novel", &input);
    let lines: Vec<Value> = ran
        .out
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines[0]["result"]["isError"], false, "{}", ran.out);
    assert_eq!(
        std::fs::read_to_string(root.join("contents/01-01.cnov")).unwrap(),
        "@01-01-01\n「さようなら」\n"
    );
    let proposal = std::fs::read_to_string(root.join("agents/proposals/P0001.md")).unwrap();
    assert!(
        proposal.contains("status: applied\n") && proposal.contains("origin: mcp:writer\n"),
        "{proposal}"
    );
    let logs: Vec<_> = std::fs::read_dir(root.join("agents/log"))
        .unwrap()
        .collect();
    assert_eq!(logs.len(), 1);
    let log = std::fs::read_to_string(logs[0].as_ref().unwrap().path()).unwrap();
    assert!(log.contains("\"event\":\"direct_write\""), "{log}");
    assert!(!log.contains("さようなら"), "{log}");
    assert_eq!(lines[1]["result"]["isError"], true);
    assert_eq!(
        lines[1]["result"]["content"][0]["text"],
        "Delegation level too low to write: stylist"
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
            "novnow mcp <dir>",
            "novnow lsp [<dir>]",
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
        (&["check"], "novnow checkにはディレクトリを指定してください"),
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
        for _ in text.matches("sonneko.github.io/novnow/guide/project_structure.html") {
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
fn binary_mcp_speaks_json_lines_on_stdio() {
    use std::io::Write as _;
    let dir = TempDir::new("cli-binary-mcp");
    minimal_project(&dir, "novel");
    let mut child = Process::new(env!("CARGO_BIN_EXE_novnow"))
        .args(["mcp", "novel"])
        .current_dir(dir.path())
        .env_clear()
        .env("HOME", dir.join("home"))
        .env("NOVNOW_RECENT_FILE", dir.join("state/recent.json"))
        .env("NOVNOW_LANG", "ja")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("novnow を起動できない");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
            .expect("write");
    }
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(lines[0].contains("\"serverInfo\""), "{stdout}");
    assert!(lines[1].contains("novnow.mail.send"), "{stdout}");
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
    // 検査は通るが、GUI を含まないビルドでは 2（GUI を含むビルドでは窓を開いてしまうので試さない）
    if !novnow::cli::GUI_AVAILABLE {
        let output = binary(&dir, &root, &["."]);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            String::from_utf8(output.stderr).expect("utf8"),
            "novnow: この novnow には GUI が含まれていません\n使い方: novnow --help\n"
        );
    }
    // ここまで履歴にも HOME にも書いていない
    assert!(!dir.join("state").exists());
    assert!(!dir.join("home").exists());
}

/// GUI を含むビルド（`ui` feature）では窓を開いてしまうので、含まないビルドだけで確かめる。
#[cfg(not(feature = "ui"))]
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
    std::fs::write(root.join("contents/01-01.cnov"), "@01-01-01\n'未登録\n").expect("write");
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

#[test]
fn zip_import_writes_only_safe_new_files_and_preserves_existing_data() {
    use novnow::adapters::zip as archive;
    let dir = TempDir::new("cli-zip-import");
    let zip = archive::export(
        &template::new_project("小説")
            .into_iter()
            .map(|item| {
                let path = ProjectPath::new(&item.path).unwrap();
                (path, item.text.into_bytes())
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    std::fs::write(dir.join("novel.zip"), zip).unwrap();
    let env = env_ja(&dir);
    let first = run(dir.path(), &env, &["import", "novel.zip", "book"]);
    assert_eq!(first.exit(), Exit::Ok, "{}", first.err);
    assert!(dir.join("book/project.toml").exists());
    dir.write("book/words.md", "# 自分の用語\n");
    let second = run(dir.path(), &env, &["import", "novel.zip", "book"]);
    assert_eq!(second.exit(), Exit::Ok, "{}", second.err);
    assert_eq!(
        std::fs::read_to_string(dir.join("book/words.md")).unwrap(),
        "# 自分の用語\n"
    );
    assert!(second.out.contains("words.md"));
    assert!(!dir.join("state").exists());
    assert_eq!(
        decide(&os_args(&["import", "one.zip", "out"]), Terminal::default()),
        Launch::Command(Command::Import {
            zip: PathBuf::from("one.zip"),
            dir: PathBuf::from("out")
        })
    );
}

#[test]
fn zip_import_rejects_bad_archive_without_creating_destination() {
    let dir = TempDir::new("cli-import-broken");
    dir.write("bad.zip", "not a zip");
    let result = run(
        dir.path(),
        &env_ja(&dir),
        &["import", "bad.zip", "destination"],
    );
    assert_eq!(result.exit(), Exit::Io);
    assert!(!dir.join("destination").exists());
    assert_eq!(
        decide(&os_args(&["import", "file.zip"]), Terminal::default()),
        Launch::Error(ArgError::MissingDir("import"))
    );
}

#[test]
fn migration_preview_is_read_only_and_import_refuses_existing_targets() {
    let dir = TempDir::new("migrate");
    let env = env_ja(&dir);
    let source = "@heading\n# memo\n「body」@speaker\n";
    dir.write("source.txt", source);
    let preview = run(
        dir.path(),
        &env,
        &["migrate", "source.txt", "book", "--preview"],
    );
    assert_eq!(preview.exit(), Exit::Ok, "{preview:?}");
    assert!(!dir.join("book").exists());
    assert!(preview.out.contains("contents/01-01.cnov"));
    let imported = run(dir.path(), &env, &["migrate", "source.txt", "book"]);
    assert_eq!(imported.exit(), Exit::Ok, "{imported:?}");
    let before = tree(&dir.join("book"));
    assert_eq!(
        run(dir.path(), &env, &["migrate", "source.txt", "book"]).exit(),
        Exit::Io
    );
    assert_eq!(tree(&dir.join("book")), before);
    assert_eq!(
        std::fs::read_to_string(dir.join("source.txt")).unwrap(),
        source
    );
    assert!(!dir.join("state/recent.json").exists());
}

#[test]
fn migration_invalid_input_never_creates_a_target() {
    let dir = TempDir::new("migrate-invalid");
    let env = env_ja(&dir);
    for (name, content) in [("empty.txt", " \n"), ("nul.txt", "a\0b")] {
        dir.write(name, content);
        assert_eq!(
            run(dir.path(), &env, &["migrate", name, "book"]).exit(),
            Exit::Io
        );
        assert!(!dir.join("book").exists());
    }
    std::fs::write(dir.join("invalid.txt"), [0xff, 0xfe]).unwrap();
    assert_eq!(
        run(dir.path(), &env, &["migrate", "invalid.txt", "book"]).exit(),
        Exit::Io
    );
    assert!(!dir.join("book").exists());
}

#[test]
fn migration_refuses_empty_directories_and_missing_parents() {
    let dir = TempDir::new("migrate-existing");
    let env = env_ja(&dir);
    dir.write("source.txt", "body\n");
    dir.mkdir("existing");
    assert_eq!(
        run(dir.path(), &env, &["migrate", "source.txt", "existing"]).exit(),
        Exit::Io
    );
    assert!(tree(&dir.join("existing")).is_empty());
    assert_eq!(
        run(dir.path(), &env, &["migrate", "source.txt", "missing/book"]).exit(),
        Exit::Io
    );
    assert!(!dir.join("missing").exists());
}

#[cfg(unix)]
#[test]
fn migration_refuses_symlink_destinations_without_touching_them() {
    let dir = TempDir::new("migrate-link");
    let env = env_ja(&dir);
    dir.write("source.txt", "body\n");
    dir.mkdir("outside");
    std::os::unix::fs::symlink(dir.join("outside"), dir.join("link")).unwrap();
    assert_eq!(
        run(dir.path(), &env, &["migrate", "source.txt", "link"]).exit(),
        Exit::Io
    );
    assert!(tree(&dir.join("outside")).is_empty());
}

#[test]
fn migration_splits_headings_and_multiple_inputs() {
    let dir = TempDir::new("migrate-split");
    let env = env_ja(&dir);
    dir.write(
        "part.txt",
        "第一章 始まり\n本文です。\n\n第二章 続き\n本文ふたたび。\n",
    );
    dir.write("extra.txt", "追加の一話。\n");
    let preview = run(
        dir.path(),
        &env,
        &["migrate", "part.txt", "extra.txt", "book", "--preview"],
    );
    assert_eq!(preview.exit(), Exit::Ok, "{preview:?}");
    assert!(preview.out.contains("2 章"), "{}", preview.out);
    assert!(!dir.join("book").exists());
    let imported = run(
        dir.path(),
        &env,
        &["migrate", "part.txt", "extra.txt", "book"],
    );
    assert_eq!(imported.exit(), Exit::Ok, "{imported:?}");
    // CLI は候補どおりに生成する: 見出しで章が分かれ、2 ファイル目は
    // 最後の章の新しい話になる。
    let chapter_two = std::fs::read_to_string(dir.join("book/contents/02-01.cnov")).unwrap();
    assert!(chapter_two.contains("本文ふたたび。"), "{chapter_two}");
    let episode_two = std::fs::read_to_string(dir.join("book/contents/02-02.cnov")).unwrap();
    assert!(episode_two.contains("追加の一話。"), "{episode_two}");
    let all = std::fs::read_to_string(dir.join("book/plots/all.cnov")).unwrap();
    assert!(all.contains("@01 始まり"), "{all}");
    assert!(all.contains("@02 続き"), "{all}");
}

#[test]
fn migration_folder_input_reads_natural_order_as_one_chapter() {
    let dir = TempDir::new("migrate-folder");
    let env = env_ja(&dir);
    dir.write("ch/2.txt", "二話目。\n");
    dir.write("ch/10.txt", "十話目。\n");
    dir.write("ch/1.txt", "一話目。\n");
    dir.write("ch/notes.bin", "binary\n");
    dir.write("ch/sub/99.txt", "サブフォルダは読まない。\n");
    dir.write("first.txt", "最初の章。\n");
    let imported = run(dir.path(), &env, &["migrate", "first.txt", "ch", "book"]);
    assert_eq!(imported.exit(), Exit::Ok, "{imported:?}");
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap();
    assert!(read("book/contents/01-01.cnov").contains("最初の章。"));
    assert!(read("book/contents/02-01.cnov").contains("一話目。"));
    assert!(read("book/contents/02-02.cnov").contains("二話目。"));
    assert!(read("book/contents/02-03.cnov").contains("十話目。"));
    assert!(!dir.join("book/contents/02-04.cnov").exists());
}

#[test]
fn migration_reads_shift_jis_files() {
    let dir = TempDir::new("migrate-sjis");
    let env = env_ja(&dir);
    // 「本文です。」の Shift_JIS バイト列。
    std::fs::write(
        dir.join("sjis.txt"),
        [
            0x96, 0x7B, 0x95, 0xB6, 0x82, 0xC5, 0x82, 0xB7, 0x81, 0x42, 0x0A,
        ],
    )
    .unwrap();
    let imported = run(dir.path(), &env, &["migrate", "sjis.txt", "book"]);
    assert_eq!(imported.exit(), Exit::Ok, "{imported:?}");
    let contents = std::fs::read_to_string(dir.join("book/contents/01-01.cnov")).unwrap();
    assert!(contents.contains("本文です。"), "{contents}");
}

// ============================================================ MCP のツール表（§19.5.1・段階③ H4）

/// `tools/call` の要求 1 件。
fn tool_call(id: u32, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

/// 応答（標準出力の 1 行 1 メッセージ）を読む。
fn responses(out: &str) -> Vec<serde_json::Value> {
    out.lines()
        .map(|line| serde_json::from_str(line).expect("一行 1 メッセージの JSON"))
        .collect()
}

#[test]
fn mcp_lists_the_whole_catalog_and_refuses_the_human_only_operations() {
    use serde_json::json;
    let dir = TempDir::new("cli-mcp-catalog");
    minimal_project(&dir, "novel");
    let input = mcp_lines(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
        // 人間の専権事項 4 件（§19.1.5）は直接呼んでも断られる
        tool_call(
            2,
            "novnow.settings.set",
            json!({"key": "theme", "value": "dark"}),
        ),
        tool_call(
            3,
            "novnow.agent.delegation_set",
            json!({"role": "writer", "level": "apply"}),
        ),
        tool_call(4, "novnow.agent.mcp_config_set", json!({"text": "{}"})),
        tool_call(5, "novnow.prompt.reset", json!({"name": "writer"})),
        // カタログの操作は 1 回分で答えが返る（§19.1.6）
        tool_call(6, "novnow.section.list", json!({})),
        tool_call(7, "novnow.nope", json!({})),
    ]);
    let ran = run_mcp(dir.path(), &env_ja(&dir), "novel", &input);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let lines = responses(&ran.out);
    assert_eq!(lines.len(), 7, "{}", ran.out);

    let tools = lines[0]["result"]["tools"].as_array().expect("tools");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    // 「M—」以外の全操作が載り、「M—」は載らない（既存 14 ツールは別枠で凍結）
    for def in novnow::app::ops::OPS {
        let name = format!("novnow.{}", def.id);
        let listed = names.contains(&name.as_str());
        if def.excepts(novnow::app::session::Interface::Mcp).is_none() {
            assert!(listed, "{name} がツール表に無い（§19.5.1）");
        } else {
            assert!(
                !listed || server::is_tool(&name),
                "{name} は MCP で提供しないのに載っている"
            );
        }
    }
    // 全ツールが JSON Schema を持つ（§19.5.3）
    assert!(
        tools
            .iter()
            .all(|tool| tool["inputSchema"]["type"] == json!("object")),
        "inputSchema が無いツールがある"
    );
    // 安全除外 4 件は登録されない（§19.1.5）
    for name in [
        "novnow.settings.set",
        "novnow.agent.delegation_set",
        "novnow.agent.mcp_config_set",
        "novnow.prompt.reset",
    ] {
        assert!(!names.contains(&name), "{name} がツール表にある");
    }
    // 直接呼び出しも断る（isError）
    for line in &lines[1..5] {
        assert_eq!(line["result"]["isError"], true, "{line}");
    }
    assert_eq!(lines[5]["result"]["isError"], false, "{}", lines[5]);
    // カタログに無い名前は未知のツール（既存の文言）
    assert_eq!(lines[6]["result"]["isError"], true, "{}", lines[6]);
    assert!(
        lines[6]["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("novnow.nope"),
        "{}",
        lines[6]
    );
}

#[test]
fn mcp_write_tools_check_the_role_and_write_nothing() {
    let dir = TempDir::new("cli-mcp-gate");
    let root = minimal_project(&dir, "novel");
    let before = std::fs::read_to_string(root.join("contents/01-01.cnov")).expect("read");
    let input = mcp_lines(&[
        // role が無い（§19.1.5「省略時は E036」）
        tool_call(
            1,
            "novnow.section.insert",
            serde_json::json!({"text": "関門を通らない本文"}),
        ),
        // 既定の委譲（propose）では role があっても書けない
        tool_call(
            2,
            "novnow.section.insert",
            serde_json::json!({"text": "まだ書けない本文", "role": "writer"}),
        ),
    ]);
    let ran = run_mcp(dir.path(), &env_ja(&dir), "novel", &input);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let lines = responses(&ran.out);
    assert_eq!(lines.len(), 2, "{}", ran.out);
    for line in &lines {
        assert_eq!(line["result"]["isError"], true, "{line}");
        assert!(
            line["result"]["content"][0]["text"]
                .as_str()
                .expect("text")
                .contains("書き込めません"),
            "E036 の文言ではない: {line}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(root.join("contents/01-01.cnov")).expect("read"),
        before,
        "関門を通らないのに書き込んだ"
    );
}

#[test]
fn mcp_plot_write_takes_outline_and_chapter_levels() {
    use serde_json::json;
    let dir = TempDir::new("cli-mcp-plot");
    let root = minimal_project(&dir, "novel");
    // writer の委譲を apply に（§15.7.6。`agents.roles` の上書き）
    dir.write(
        "novel/project.toml",
        "schema_version = 2\ntitle = \"星屑の時計\"\n\n[agents.roles]\nwriter = \"apply\"\n",
    );
    dir.write("novel/plots/01.cnov", "@01\nsummary: 章のプロット\n;\n");
    dir.write("novel/plots/01-01.cnov", "@01-01-01\n話のプロット\n");
    let input = mcp_lines(&[
        tool_call(
            1,
            "novnow.plot.write",
            json!({"role": "writer", "level": "outline", "text": "@01\nsummary: 全体を書き換えた\n;\n"}),
        ),
        tool_call(
            2,
            "novnow.plot.write",
            json!({"role": "writer", "level": "chapter", "chapter": 1, "text": "@01\nsummary: 章を書き換えた\n;\n"}),
        ),
        // 既存の episode は凍結された実装のまま（§19.8.3）
        tool_call(
            3,
            "novnow.plot.write",
            json!({"role": "writer", "level": "episode", "scene": "@01-01-01-01", "text": "話を書き換えた\n"}),
        ),
        // 知らない level は正規操作の引数エラー（§19.5.2）。どのファイルも変わらない
        tool_call(
            4,
            "novnow.plot.write",
            json!({"role": "writer", "level": "scene", "text": "だめ\n"}),
        ),
    ]);
    let ran = run_mcp(dir.path(), &env_ja(&dir), "novel", &input);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let lines = responses(&ran.out);
    assert_eq!(lines.len(), 4, "{}", ran.out);
    for line in &lines[..3] {
        assert_eq!(line["result"]["isError"], false, "{line}");
    }
    assert_eq!(lines[3]["result"]["isError"], true, "{}", lines[3]);
    let outline = std::fs::read_to_string(root.join("plots/all.cnov")).expect("read");
    assert!(outline.contains("全体を書き換えた"), "{outline}");
    let chapter = std::fs::read_to_string(root.join("plots/01.cnov")).expect("read");
    assert!(chapter.contains("章を書き換えた"), "{chapter}");
    let episode = std::fs::read_to_string(root.join("plots/01-01.cnov")).expect("read");
    assert!(episode.contains("話を書き換えた"), "{episode}");
    // 直接書き込みは監査ログに残る（§15.8.4）
    let logged = tree(&root.join("agents"))
        .values()
        .any(|bytes| String::from_utf8_lossy(bytes).contains("direct_write"));
    assert!(logged, "監査ログに direct_write が無い");
}

// ============================================================ novnow lsp

/// `novnow lsp` を、標準入力の代わりに `input`（枠つきの JSON-RPC）を渡して走らせる。
fn run_lsp(cwd: &Path, env: &FakeEnv, dir: &str, input: &str) -> Ran {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut reader = std::io::Cursor::new(input.as_bytes().to_vec());
    let outcome = {
        let mut cli = Cli::new(
            env,
            &mut out,
            &mut err,
            cwd.to_path_buf(),
            Terminal::default(),
        );
        cli.set_input(&mut reader);
        cli::run(&os_args(&["lsp", dir]), &mut cli)
    };
    Ran {
        outcome,
        out: String::from_utf8(out).expect("stdout は UTF-8"),
        err: String::from_utf8(err).expect("stderr は UTF-8"),
    }
}

/// 枠つきの 1 通話分を作る。
fn lsp_frames(messages: &[serde_json::Value]) -> String {
    messages
        .iter()
        .map(|message| novnow::lsp::framing::encode(&message.to_string()))
        .collect()
}

/// 標準出力を枠ごとに分解する。
fn lsp_messages(stdout: &str) -> Vec<serde_json::Value> {
    let mut cursor = std::io::Cursor::new(stdout.as_bytes().to_vec());
    let mut messages = Vec::new();
    while let Some(body) = novnow::lsp::framing::read_message(&mut cursor) {
        messages.push(serde_json::from_str(&body).expect("JSON"));
    }
    messages
}

#[test]
fn lsp_checks_the_project_first() {
    let dir = TempDir::new("cli-lsp-broken");
    spec_like_incomplete(&dir, "broken");
    let ran = run_lsp(dir.path(), &env_ja(&dir), "broken", "");
    assert_eq!(ran.exit(), Exit::Project, "§14.2 の検査が先（{}）", ran.err);
    assert_eq!(ran.out, "", "プロトコル以外を標準出力に出さない");
}

#[test]
fn lsp_speaks_framed_jsonrpc_over_stdio() {
    let dir = TempDir::new("cli-lsp");
    minimal_project(&dir, "novel");
    let project = dir.join("novel");
    let bodies = lsp_frames(&[
        serde_json::json!({"id": 1, "jsonrpc": "2.0", "method": "initialize", "params": {"capabilities": {}}}),
        serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        serde_json::json!({"id": 2, "jsonrpc": "2.0", "method": "workspace/executeCommand",
               "params": {"arguments": [{}], "command": "api.list"}}),
        serde_json::json!({"id": 3, "jsonrpc": "2.0", "method": "textDocument/noSuchFeature", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
    ]);
    let ran = run_lsp(
        dir.path(),
        &env_ja(&dir),
        project.to_str().expect("path"),
        &bodies,
    );
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let messages = lsp_messages(&ran.out);
    assert!(messages.len() >= 3, "枠が読めない: {:?}", ran.out);
    let capabilities = &messages[0]["result"]["capabilities"];
    assert_eq!(capabilities["textDocumentSync"], serde_json::json!(1));
    assert_eq!(
        capabilities["executeCommandProvider"]["commands"]
            .as_array()
            .expect("commands")
            .len(),
        123,
        "操作 121 件 + 特別な 2 つ"
    );
    let api = messages
        .iter()
        .find(|message| message["id"] == serde_json::json!(2))
        .expect("api.list の応答");
    assert_eq!(
        api["result"]["count"],
        serde_json::json!(121),
        "§19.7 の全行"
    );
    let unknown = messages
        .iter()
        .find(|message| message["id"] == serde_json::json!(3))
        .expect("未知のメソッドの応答");
    assert_eq!(unknown["error"]["code"], serde_json::json!(-32601));
    assert_eq!(ran.err, "", "標準エラーに何も出さない");
}

// ============================================================ novnow <domain> <verb>（§19.6）

/// `--json` の封筒を読む（§19.6.2）。
fn envelope(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|error| panic!("封筒でない: {error} {stdout:?}"))
}

#[test]
fn exit_codes_come_only_from_the_documented_table() {
    use novnow::app::error::Class;
    use novnow::cli::json::exit_of;
    // §19.1.7 の表（CLI 終了コード）。§14.2.5 の 0・2・3・4・5 だけで、新しい値は足さない。
    let table = [
        (Class::Usage, Exit::Usage),
        (Class::Project, Exit::Project),
        (Class::Io, Exit::Io),
        (Class::Validation, Exit::Diagnostics),
        (Class::Policy, Exit::Diagnostics),
        (Class::NotFound, Exit::Project),
        (Class::Conflict, Exit::Io),
        (Class::Timeout, Exit::Io),
        (Class::Cancelled, Exit::Ok),
    ];
    for (class, exit) in table {
        assert_eq!(exit_of(&class), exit, "{class:?}");
    }
    assert_eq!(table.len(), 9, "§19.1.7 の 9 分類すべて");
}

#[test]
fn the_domain_verb_grammar_reaches_every_catalog_operation() {
    for def in novnow::app::ops::OPS {
        let (domain, _) = def.id.split_once('.').expect("id");
        if novnow::cli::SUBCOMMANDS.contains(&domain) {
            // 既存のサブコマンド語が優先する（§19.6.1）。`export.text` は `novnow export`、
            // `api.list` は `novnow api list` から届く（下のテストが見る）。
            continue;
        }
        // 操作はプロジェクトを変える（`section.delete`・`document.rename` など）ので、
        // 1 件ずつ新しいプロジェクトで見る。
        let dir = TempDir::new("cli-domain-verb");
        minimal_project(&dir, "novel");
        let env = env_ja(&dir);
        let (_, verb) = def.id.split_once('.').expect("id");
        let ran = run(dir.path(), &env, &[domain, verb, "novel", "--json"]);
        assert!(
            !ran.out.trim().is_empty(),
            "{}: 何も出ない（{:?} {}）",
            def.id,
            ran.exit(),
            ran.err
        );
        let value = envelope(&ran.out);
        assert_eq!(value["op"], serde_json::json!(def.id), "{}", def.id);
        assert!(
            value["ok"].is_boolean(),
            "{}: `ok` が無い {:?}",
            def.id,
            ran.out
        );
        if value["ok"] == serde_json::json!(true) {
            assert!(
                value["data"].is_object() || value["data"].is_array(),
                "{}",
                def.id
            );
        } else {
            for key in ["code", "kind", "message"] {
                assert!(
                    value["error"][key].is_string(),
                    "{}: `error.{key}` が無い {:?}",
                    def.id,
                    ran.out
                );
            }
        }
        // §14.2.5: 終了コードは 0・2・3・4・5 だけ。
        assert!(
            matches!(
                ran.exit(),
                Exit::Ok | Exit::Usage | Exit::Project | Exit::Io | Exit::Diagnostics
            ),
            "{}: {:?}",
            def.id,
            ran.exit()
        );
    }
}

#[test]
fn the_json_envelope_carries_the_error_model() {
    let dir = TempDir::new("cli-envelope");
    minimal_project(&dir, "novel");
    let env = env_ja(&dir);
    // 対象 ID が無い → not_found（§19.1.7: 終了コード 3）
    let ran = run(
        dir.path(),
        &env,
        &["materials", "get", "novel", "--json", "--id", "M9999"],
    );
    assert_eq!(ran.exit(), Exit::Project, "{} {}", ran.out, ran.err);
    let value = envelope(&ran.out);
    assert_eq!(value["ok"], serde_json::json!(false));
    assert_eq!(value["op"], serde_json::json!("materials.get"));
    assert_eq!(value["error"]["code"], serde_json::json!("not_found"));
    assert!(!value["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .is_empty());
    // 引数の形が誤り → usage（2）。範囲に当てはまる節が無いのも §6.3 では usage。
    let ran = run(
        dir.path(),
        &env,
        &["section", "get", "novel", "--json", "--id"],
    );
    assert_eq!(ran.exit(), Exit::Usage, "{} {}", ran.out, ran.err);
    let ran = run(
        dir.path(),
        &env,
        &["section", "get", "novel", "--json", "--id", "99-99-99-99"],
    );
    assert_eq!(ran.exit(), Exit::Usage, "{} {}", ran.out, ran.err);
    assert_eq!(
        envelope(&ran.out)["error"]["code"],
        serde_json::json!("usage")
    );
    // Zip が無い → io（4。§19.1.7）
    let ran = run(
        dir.path(),
        &env,
        &[
            "project",
            "import_zip",
            "novel",
            "--json",
            "--path",
            "nope.zip",
        ],
    );
    assert_eq!(ran.exit(), Exit::Io, "{} {}", ran.out, ran.err);
    assert_eq!(
        envelope(&ran.out)["error"]["code"],
        serde_json::json!("io"),
        "{}",
        ran.out
    );
    // CLI は人間経路なので書き込める（MCP では同じ操作が関門で E036＝policy。§19.5.2）。
    // policy → 5 の写像は上の `exit_codes_come_only_from_the_documented_table` が見る。
    let ran = run(
        dir.path(),
        &env,
        &[
            "document",
            "write",
            "novel",
            "--json",
            "--path",
            "contents/01-01.cnov",
            "--text",
            "書き換え",
        ],
    );
    assert_eq!(ran.exit(), Exit::Ok, "{} {}", ran.out, ran.err);
    assert_eq!(
        envelope(&ran.out)["data"]["save"]["written"],
        serde_json::json!(["contents/01-01.cnov"]),
        "{}",
        ran.out
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("novel/contents/01-01.cnov")).expect("本文"),
        "書き換え",
        "書いていない"
    );
}

#[test]
fn the_operand_can_come_from_stdin() {
    let dir = TempDir::new("cli-operand");
    minimal_project(&dir, "novel");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut reader = std::io::Cursor::new("星図".as_bytes().to_vec());
    let env = env_ja(&dir);
    let outcome = {
        let mut cli = Cli::new(
            &env,
            &mut out,
            &mut err,
            dir.path().to_path_buf(),
            Terminal::default(),
        );
        cli.set_input(&mut reader);
        cli::run(
            &os_args(&["search", "project", "novel", "-", "--json"]),
            &mut cli,
        )
    };
    let stdout = String::from_utf8(out).expect("stdout");
    assert_eq!(outcome, Outcome::Exit(Exit::Ok), "{stdout}");
    let value = envelope(&stdout);
    assert_eq!(value["op"], serde_json::json!("search.project"));
    assert_eq!(value["data"]["query"], serde_json::json!("星図"));
}

#[test]
fn api_list_shows_the_catalog_and_takes_json() {
    let dir = TempDir::new("cli-api");
    minimal_project(&dir, "novel");
    let env = env_ja(&dir);
    // 人間可読: 1 行 1 操作
    let ran = run(dir.path(), &env, &["api", "list"]);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    assert_eq!(
        ran.out.lines().count(),
        novnow::app::ops::OPS.len(),
        "カタログの全行が出ていない"
    );
    assert!(ran.out.contains("lang.hover"), "{}", ran.out);
    // `--json`: 封筒の中の `data` が `api.list` そのもの（§19.8.1）
    let ran = run(dir.path(), &env, &["api", "list", "--json"]);
    assert_eq!(ran.exit(), Exit::Ok, "{}", ran.err);
    let data = &envelope(&ran.out)["data"];
    assert_eq!(
        data["count"],
        serde_json::json!(novnow::app::ops::OPS.len())
    );
    assert_eq!(
        data["interfaces"],
        serde_json::json!(["rust", "python", "lsp", "mcp", "cli"]),
    );
    let operations = data["operations"].as_array().expect("operations");
    for (index, def) in novnow::app::ops::OPS.iter().enumerate() {
        assert_eq!(operations[index]["id"], serde_json::json!(def.id));
    }
    // 動詞が違えば 2
    assert_eq!(
        run(dir.path(), &env, &["api", "show"]).exit(),
        Exit::Usage,
        "`api list` だけ"
    );
    // `--lang en`（§19.1.8）
    let ja = run(dir.path(), &env, &["api", "list"]);
    let en = run(dir.path(), &env, &["api", "list", "--lang", "en"]);
    assert_eq!(en.exit(), Exit::Ok, "{}", en.err);
    assert!(
        en.out.contains("Project information"),
        "説明文が英語でない: {}",
        &en.out[..200.min(en.out.len())]
    );
    assert_ne!(ja.out, en.out, "§19.1.8 の `--lang` が効いていない");
}

#[test]
fn a_directory_named_like_a_domain_still_opens_with_the_slash_form() {
    let dir = TempDir::new("cli-project-dir");
    minimal_project(&dir, "project");
    let env = env_ja(&dir);
    // §14.1: `./` を付けるとディレクトリ。`novnow project` だけでは動詞が無く GUI 側の解釈。
    let ran = run(dir.path(), &env, &["./project"]);
    assert!(
        matches!(ran.outcome, Outcome::Gui(Some(_))) || ran.exit() == Exit::Ok,
        "{:?} {}",
        ran.outcome,
        ran.err
    );
}

#[test]
fn the_frozen_subcommands_still_win_over_domain_names() {
    let dir = TempDir::new("cli-frozen");
    minimal_project(&dir, "novel");
    let env = env_ja(&dir);
    // `export` は §17.2 の既存サブコマンド（`export.text` の優先形。§19.6.1 の表）
    let frozen = run(dir.path(), &env, &["export", "novel"]);
    assert_eq!(frozen.exit(), Exit::Ok, "{}", frozen.err);
    // 既存コマンド語が優先するので、`export text` は動詞として読まない（§19.6.1）。
    assert_eq!(
        run(dir.path(), &env, &["export", "text", "novel"]).exit(),
        Exit::Usage,
        "凍結した `export` が勝つ"
    );
    // `check` も同じ（`project.validate` の優先形）。正規形はドメイン名から届く。
    assert_eq!(run(dir.path(), &env, &["check", "novel"]).exit(), Exit::Ok);
    let canonical = run(
        dir.path(),
        &env,
        &["project", "validate", "novel", "--json"],
    );
    assert_eq!(
        canonical.exit(),
        Exit::Ok,
        "{} {}",
        canonical.out,
        canonical.err
    );
    assert_eq!(
        envelope(&canonical.out)["op"],
        serde_json::json!("project.validate")
    );
}

#[test]
fn api_reserved_words_can_be_opened_as_explicit_relative_directories() {
    for name in ["api", "lsp", "project", "section"] {
        let path = format!("./{name}");
        assert_eq!(
            decide(&os_args(&[&path]), Terminal::default()),
            Launch::Open(PathBuf::from(&path))
        );
    }
}
