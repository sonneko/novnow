//! novnow エントリポイント。
//!
//! 使い方:
//!   novnow <project-dir>          # 既存プロジェクトを開く（不正ならエラー終了）
//!   novnow --init <project-dir>   # 新規プロジェクトを雛形から作成する
#![allow(dead_code)]

mod app;
mod fonts;
mod panes;
mod project;
mod text;
mod types;

use std::path::PathBuf;

/// eframe 0.27 の `run_native` は `eframe::Result<()>` を返す。
fn main() -> eframe::Result<()> {
    // --- コマンドライン解析（プロジェクトルートは必須） --------------------
    let mut init = false;
    let mut positional: Vec<String> = Vec::new();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--init" | "-i" => init = true,
            "-h" | "--help" => {
                print_usage();
                return Ok(());
            }
            other => positional.push(other.to_string()),
        }
    }

    let target: PathBuf = match positional.first() {
        Some(p) => PathBuf::from(p),
        None => {
            print_usage();
            std::process::exit(2);
        }
    };

    // 非同期ファイル I/O 用の tokio ランタイムを 1 つだけ生成してアプリ全体で共有する。
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("novnow-io")
        .build()
        .expect("tokio ランタイムの生成に失敗");

    // 起動時にプロジェクトを読み込む。ファイルが揃っていなければエラーで終了する。
    let state = if init {
        match runtime.block_on(project::loader::scaffold_project(&target)) {
            Ok(()) => println!("プロジェクトを初期化しました: {}", target.display()),
            Err(e) => {
                eprintln!("初期化に失敗しました: {e:#}");
                std::process::exit(1);
            }
        }
        match runtime.block_on(project::loader::load_project(target.clone())) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("プロジェクトを読み込めませんでした: {e:#}");
                std::process::exit(1);
            }
        }
    } else {
        match runtime.block_on(project::loader::load_project(target.clone())) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("プロジェクトを読み込めませんでした: {e:#}");
                eprintln!(
                    "ヒント: 新規作成する場合は `novnow --init {}` を実行してください。",
                    target.display()
                );
                std::process::exit(1);
            }
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title(format!("novnow — {}", state.title)),
        ..Default::default()
    };

    // eframe 0.27 の AppCreator は `Box<dyn FnOnce(&CreationContext) -> Box<dyn App>>`。
    eframe::run_native(
        "novnow",
        native_options,
        Box::new(move |cc| {
            fonts::setup_japanese_fonts(&cc.egui_ctx);
            Box::new(app::NovnowApp::new(cc, runtime, state))
        }),
    )
}

fn print_usage() {
    eprintln!("novnow — 小説プロジェクト向け執筆 IDE");
    eprintln!();
    eprintln!("usage:");
    eprintln!("  novnow <project-dir>         既存プロジェクトを開く");
    eprintln!("  novnow --init <project-dir>  新規プロジェクトを作成");
    eprintln!();
    eprintln!("例: novnow ./");
}
