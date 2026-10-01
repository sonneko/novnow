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
mod storage;
mod text;
mod types;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use std::path::PathBuf;

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
        match project::loader::load_project(target.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("プロジェクトを読み込めませんでした: {e:#}");
                std::process::exit(1);
            }
        }
    } else {
        match project::loader::load_project(target.clone()) {
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

#[cfg(not(target_arch = "wasm32"))]
fn print_usage() {
    eprintln!("novnow — 小説プロジェクト向け執筆 IDE");
    eprintln!();
    eprintln!("usage:");
    eprintln!("  novnow <project-dir>         既存プロジェクトを開く");
    eprintln!("  novnow --init <project-dir>  新規プロジェクトを作成");
    eprintln!();
    eprintln!("例: novnow ./");
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let runner = eframe::WebRunner::new();
        let state = project::loader::scaffold_in_memory_state();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("tokio runtime failed");

        let _ = runner
            .start(
                "the_canvas_id",
                web_options,
                Box::new(|cc| {
                    fonts::setup_japanese_fonts(&cc.egui_ctx);
                    let mut app = app::NovnowApp::new(cc, runtime, state);
                    if let Some(p) = app.state.contents_files.first().cloned() {
                        app.open_file(&p);
                    }
                    Box::new(app)
                }),
            )
            .await;
    });
}
