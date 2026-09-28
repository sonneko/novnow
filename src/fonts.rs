//! システム日本語フォントの検出と egui への登録。
//!
//! `fontdb` でプラットフォームのシステムフォントをスキャンし、
//! 日本語対応フォントを見つけて egui のフォールバックチェーンに追加する。

use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use fontdb::{Database, Family, Query, Style, Stretch, Weight};

/// 起動時に呼び出し、日本語フォントを検出して egui に登録する。
///
/// 以下の優先順位でフォントを探す:
/// 1. Noto Sans CJK JP
/// 2. Meiryo (Windows)
/// 3. MS PGothic (Windows)
/// 4. Hiragino Sans / Hiragino Kaku Gothic (macOS)
/// 5. Yu Gothic (Windows)
/// 6. TakaoPGothic / IPAGothic / VL PGothic (Linux)
/// 7. 汎用 sans-serif
///
/// 見つからない場合は標準エラーに警告を出す（panic しない）。
pub fn setup_japanese_fonts(ctx: &egui::Context) {
    let mut db = Database::new();
    db.load_system_fonts();

    let families: Vec<Family> = vec![
        Family::Name("Noto Sans CJK JP"),
        Family::Name("Meiryo"),
        Family::Name("MS PGothic"),
        Family::Name("Hiragino Sans"),
        Family::Name("Hiragino Kaku Gothic ProN"),
        Family::Name("Yu Gothic"),
        Family::Name("TakaoPGothic"),
        Family::Name("IPAGothic"),
        Family::Name("VL PGothic"),
        Family::Name("Noto Sans JP"),
        Family::SansSerif,
    ];

    let query = Query {
        families: &families,
        weight: Weight::NORMAL,
        stretch: Stretch::Normal,
        style: Style::Normal,
    };

    let Some(id) = db.query(&query) else {
        eprintln!(
            "警告: 日本語フォントが見つかりません。"
        );
        eprintln!("  日本語表示には以下のいずれかのフォントインストールが必要です:");
        eprintln!("    - Noto Sans CJK JP / Noto Sans JP");
        eprintln!("    - Meiryo / Yu Gothic / MS PGothic (Windows)");
        eprintln!("    - Hiragino Sans (macOS)");
        eprintln!("    - TakaoPGothic / IPAGothic / VL PGothic (Linux)");
        return;
    };

    // fontdb からフォントバイナリをコピーして取得する。
    // with_face_data はクロージャの戻り値を Option<T> で返す。
    let Some((bytes, index)) = db.with_face_data(id, |data, idx| (data.to_vec(), idx)) else {
        eprintln!("警告: 日本語フォントの読み込みに失敗しました");
        return;
    };

    let mut fonts = FontDefinitions::default();

    // 日本語フォントを "japanese" として登録
    fonts.font_data.insert(
        "japanese".to_string(),
        FontData {
            font: std::borrow::Cow::Owned(bytes),
            index,
            tweak: FontTweak::default(),
        },
    );

    // Proportional / Monospace のフォールバック末尾に追加。
    // これにより既定のラテンフォントで描画できない文字（漢字等）だけが
    // 自動的に日本語フォントで描画される。
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .push("japanese".to_string());

    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("japanese".to_string());

    ctx.set_fonts(fonts);
}
