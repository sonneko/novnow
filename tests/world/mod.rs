//! M4 の機能テストで共有するプロジェクト（`features::Project`）。
//!
//! 本文（§5.2.3）・話プロット（§5.4.3）・全体プロット（§5.3.3）・用語（§5.6）・資料の索引（§5.8.2）は
//! **仕様書の例をそのまま**使う（`tests/golden/**` と同じ入力）。`characters.md` だけは §5.5.7 の例が
//! 恩田恵で、ほかの例（アルスとリリア）と人物が合わないので、§5.5 の書式どおりにアルスとリリアを
//! 書いたものに**差し替えた**（モード ID は §5.2.3 / §5.4.3 / §5.3.3 の例が使う A / B / C）。

#![allow(dead_code)]

use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::Project as Settings;

pub const CHARACTERS: &str = "# アルス
reading: あるす
alias: 錬金少年
default-mode: A

## modes

### A 探究
range: @01 ..

### B 補習
range: @02 ..

## tone
「*だ」

## relations
'リリア: 幼馴染。

# リリア
reading: りりあ
default-mode: A

## modes

### A 呆れ
range: @01 ..

### B 呆れ（強）
range: @01 ..

### C 好奇心
range: @01 ..
「*わよ」

## tone
「*のね」
";

pub const WORDS: &str = include_str!("../golden/words_md/spec_5_6.txt");
pub const INDEX: &str = include_str!("../golden/materials_index/spec_5_8_2.txt");
pub const CONTENTS: &str = include_str!("../golden/contents/spec_5_2_3.txt");
pub const EPISODE: &str = include_str!("../golden/plot_episode/spec_5_4_3.txt");
pub const ALL: &str = include_str!("../golden/plot_outline/spec_5_3_3.txt");

pub const CONTENTS_PATH: &str = "contents/01-04.cnov";
pub const EPISODE_PATH: &str = "plots/01-04.cnov";
pub const ALL_PATH: &str = "plots/all.cnov";

/// 仕様書の例でできたプロジェクト。
pub fn world() -> Project {
    Project::with_documents(
        Settings::default(),
        [
            Document::new("characters.md", CHARACTERS),
            Document::new("words.md", WORDS),
            Document::new("materials/index.json", INDEX),
            Document::new(CONTENTS_PATH, CONTENTS),
            Document::new(EPISODE_PATH, EPISODE),
            Document::new(ALL_PATH, ALL),
        ],
    )
}

/// `world()` に文書を 1 つ足した（置き換えた）もの。
pub fn world_with(path: &str, text: &str) -> Project {
    let mut project = world();
    project.insert(Document::new(path, text));
    project
}

/// `project` の `path` の文書。
pub fn doc(project: &Project, path: &str) -> Document {
    project.document(path).cloned().expect(path)
}

/// `text[range]`（byte 位置を手で数えない、handoff §4.2）。
pub fn slice(text: &str, range: &std::ops::Range<usize>) -> String {
    text.get(range.clone())
        .unwrap_or("<out of range>")
        .to_string()
}
