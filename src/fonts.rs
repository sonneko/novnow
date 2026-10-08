//! システム日本語フォントの検出と egui への登録。
//!
//! `fontdb` でプラットフォームのシステムフォントをスキャンし、
//! 見つからない場合（WASM 環境など）は組み込みの IPA ゴシックフォントをフォールバックとして使用する。

use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use fontdb::{Database, Family, Query, Style, Stretch, Weight};

pub fn setup_japanese_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut loaded = false;

    // まずシステムフォントを走査する
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

    if let Some(id) = db.query(&query) {
        if let Some((bytes, index)) = db.with_face_data(id, |data, idx| (data.to_vec(), idx)) {
            fonts.font_data.insert(
                "japanese".to_string(),
                FontData {
                    font: std::borrow::Cow::Owned(bytes),
                    index,
                    tweak: FontTweak::default(),
                },
            );
            loaded = true;
        }
    }

    // システムフォントが見つからない場合（WASM等）は埋め込みIPAゴシックを使用
    if !loaded {
        let embedded = include_bytes!("../assets/ipag.ttf");
        fonts.font_data.insert(
            "japanese".to_string(),
            FontData {
                font: std::borrow::Cow::Borrowed(embedded),
                index: 0,
                tweak: FontTweak::default(),
            },
        );
    }

    // Proportional / Monospace のフォールバック末尾に追加。
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
