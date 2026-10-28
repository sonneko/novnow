//! ContextPacket（§15.4）のテスト: 固定の構造・中身・決定論・予算と切り詰め・外したブロック。
//!
//! プロジェクトは M4 と同じ仕様書の例（`tests/world`）に、既定のプロンプト（`core::template`）を足したもの。

mod world;

use novnow::core::agents::files;
use novnow::core::aicontext::budget::{self, Block, Part, Style};
use novnow::core::aicontext::packet::{self, Inputs, Packet, UNWRITTEN};
use novnow::core::features::Project;
use novnow::core::id::SceneId;
use novnow::core::model::delegation::Delegation;
use novnow::core::model::document::Document;
use novnow::core::model::task::Task;
use novnow::core::resolve::context_rules::PacketBlock;
use novnow::core::template::DEFAULT_PROMPTS;

const NOW: &str = "2026-10-14T09:00:00+09:00";

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId::new(chapter, episode, scene, section).unwrap()
}

fn project() -> Project {
    let mut project = world::world();
    for (path, text) in DEFAULT_PROMPTS {
        project.insert(Document::new(*path, *text));
    }
    project
}

fn build(project: &Project, role: &str, scene: SceneId, excluded: &[PacketBlock]) -> Packet {
    packet::build(&Inputs {
        project,
        role,
        scene,
        delegation: Delegation::Propose,
        generated_at: NOW,
        title: "星屑",
        excluded,
        task: None,
        comment: "",
        examples: &[],
    })
}

fn body(packet: &Packet, kind: PacketBlock) -> String {
    packet.block(kind).unwrap().body()
}

#[test]
fn the_packet_has_the_header_and_the_nine_fixed_headings_in_order() {
    let project = project();
    let packet = build(&project, "writer", id(1, 4, 2, 2), &[]);
    let text = packet.text();
    assert!(
        text.starts_with(
            "# novnow context\nscene: 01-04-02-02\nrole: writer\ndelegation: propose\ngenerated_at: 2026-10-14T09:00:00+09:00\n\n## 1 project\n"
        ),
        "{text}"
    );
    let mut at = 0;
    for kind in PacketBlock::ALL {
        let found = text[at..]
            .find(&format!("\n{}\n", kind.heading()))
            .unwrap_or_else(|| panic!("{} missing or out of order:\n{text}", kind.heading()));
        at += found + 1;
    }
    assert_eq!(packet.blocks.len(), 9);
}

#[test]
fn the_writer_sees_the_whole_plot_as_one_summary_line() {
    // §15.4.5: writer のパケットでは全体プロットが 1 行（当該章の `summary`）に縮む。goal / turn / 伏線などは入らない
    let project = project();
    let packet = build(&project, "writer", id(1, 4, 2, 2), &[]);
    let outline = body(&packet, PacketBlock::Outline);
    let all: Vec<&str> = outline
        .lines()
        .filter(|line| line.starts_with("all:"))
        .collect();
    assert_eq!(
        all,
        ["all: 'アルス が地下遺跡に惹かれ、'リリア を巻き込んで探索を始める導入章。"],
        "{outline}"
    );
    for absent in ["二人の関係性と", "渋々ながら同行", "地下遺跡の封印紋様"]
    {
        assert!(!outline.contains(absent), "{outline}");
    }
}

#[test]
fn the_blocks_hold_the_section_its_neighbors_the_cast_the_lexicon_and_the_materials() {
    let project = project();
    let packet = build(&project, "writer", id(1, 4, 2, 2), &[]);
    let target = body(&packet, PacketBlock::Target);
    assert!(
        target.starts_with(
            "current:\n「旧校舎の地下遺跡に眠っているっていう'『星屑の時計』の設計図だ」@アルス"
        ),
        "{target}"
    );
    assert!(
        target.contains("before 01-04-02-01:\n「やっぱり'アルス、ここにいたのね」@リリア:A"),
        "{target}"
    );
    assert!(
        target.contains("after 01-04-02-03:\n「……付き合ってあげるわよ」@リリア:B"),
        "{target}"
    );
    let plot = body(&packet, PacketBlock::Plot);
    assert!(
        plot.starts_with("scene:\n@01-04-02 図書室の攻防\ncast: 'アルス 'リリア"),
        "{plot}"
    );
    assert!(plot.contains("section:\n'アルス は地下遺跡の'星屑の時計 の謎を解こうとしていると明かす。\nmaterial: 'M0137"), "{plot}");
    // 5 cast: 有効モードだけ（§15.4.4）
    let cast = body(&packet, PacketBlock::Cast);
    assert!(cast.contains("- name: アルス\n  mode: A 探究"), "{cast}");
    assert!(cast.contains("- name: リリア"), "{cast}");
    assert!(!cast.contains("C 好奇心"), "only the active mode: {cast}");
    // 6 lexicon: 本文と節のプロットに出る用語
    let lexicon = body(&packet, PacketBlock::Lexicon);
    assert!(
        lexicon.contains("- 星屑の時計: 旧校舎の地下遺跡に眠るとされる幻の魔導具。"),
        "{lexicon}"
    );
    // 7 materials: 話プロットの material: が明示参照する資料
    let materials = body(&packet, PacketBlock::Materials);
    assert!(materials.starts_with("- M0137"), "{materials}");
    // 8 constraints: ## output と出力契約と機械的制約
    let constraints = body(&packet, PacketBlock::Constraints);
    assert!(
        constraints.starts_with("output:\n- 本文だけを出力する。"),
        "{constraints}"
    );
    assert!(constraints.contains("contract:\n"), "{constraints}");
    assert!(constraints.contains("chars: "), "{constraints}");
    // 1 project: 題と文体規約（H1 とコメントを除く）
    let project_block = body(&packet, PacketBlock::Project);
    assert!(
        project_block.starts_with("title: 星屑\nstyle:\n"),
        "{project_block}"
    );
    assert!(!project_block.contains("# style"), "{project_block}");
}

#[test]
fn the_length_field_of_the_plot_section_is_the_writer_target() {
    let project = project();
    let packet = build(&project, "writer", id(1, 4, 2, 3), &[]);
    assert_eq!(packet.target_chars, Some(500));
    assert!(body(&packet, PacketBlock::Constraints).contains("chars: 500 (+-20%)"));
    assert!(packet.request().user.contains("500"));
}

#[test]
fn an_unwritten_section_says_so_and_takes_its_neighbors_by_id() {
    let project = project();
    let packet = build(&project, "writer", id(1, 4, 2, 4), &[]);
    let target = body(&packet, PacketBlock::Target);
    assert!(
        target.starts_with(&format!("current:\n{UNWRITTEN}")),
        "{target}"
    );
    assert!(target.contains("before 01-04-02-02:"), "{target}");
    assert!(target.contains("before 01-04-02-03:"), "{target}");
    assert!(!target.contains("after "), "{target}");
    // 4 節目は state: fixed（§5.4.3 の例）
    assert!(packet.fixed);
    assert!(body(&packet, PacketBlock::Constraints).contains("state: fixed"));
}

#[test]
fn the_packet_is_deterministic_and_the_hash_ignores_generated_at() {
    let project = project();
    let first = build(&project, "writer", id(1, 4, 2, 2), &[]);
    let second = build(&project, "writer", id(1, 4, 2, 2), &[]);
    assert_eq!(first.text(), second.text());
    assert_eq!(first.hash(), second.hash());
    assert!(first.hash().starts_with("sha256:"));
    let later = packet::build(&Inputs {
        project: &project,
        role: "writer",
        scene: id(1, 4, 2, 2),
        delegation: Delegation::Propose,
        generated_at: "2030-01-01T00:00:00+09:00",
        title: "星屑",
        excluded: &[],
        task: None,
        comment: "",
        examples: &[],
    });
    assert_ne!(first.text(), later.text());
    assert_eq!(first.hash(), later.hash());
    // 本文が変われば変わる
    let mut edited = self::project();
    edited.insert(Document::new(
        world::CONTENTS_PATH,
        world::CONTENTS.replace("設計図だ", "設計図なんだ"),
    ));
    assert_ne!(
        build(&edited, "writer", id(1, 4, 2, 2), &[]).hash(),
        first.hash()
    );
}

#[test]
fn excluded_blocks_keep_only_their_heading_and_mandatory_blocks_cannot_be_excluded() {
    let project = project();
    let packet = build(
        &project,
        "writer",
        id(1, 4, 2, 2),
        &[
            PacketBlock::Lexicon,
            PacketBlock::Materials,
            PacketBlock::Project,
            PacketBlock::Constraints,
        ],
    );
    assert!(packet.block(PacketBlock::Lexicon).unwrap().excluded);
    assert_eq!(body(&packet, PacketBlock::Lexicon), "");
    assert_eq!(body(&packet, PacketBlock::Materials), "");
    assert!(packet
        .text()
        .contains("## 6 lexicon\n\n## 7 materials\n\n## 8 constraints\n"));
    assert!(!packet.block(PacketBlock::Project).unwrap().excluded);
    assert!(!body(&packet, PacketBlock::Constraints).is_empty());
}

#[test]
fn over_budget_blocks_are_trimmed_from_the_end_with_a_marker_and_the_never_trimmed_parts_stay() {
    let mut project = project();
    // 資料の本文を長くする（7 materials は最初に削られる）
    project.insert(Document::new(
        "materials/docs/M0137-kagaku-koshien.md",
        format!(
            "# 星屑の時計\n<!-- novnow:M0137 -->\n\n{}\n",
            "あ".repeat(5000)
        ),
    ));
    let mut settings = project.settings().clone();
    settings.agents.max_chars_per_request = 2500;
    project.set_settings(settings);
    let packet = build(&project, "writer", id(1, 4, 2, 2), &[]);
    assert!(
        packet.size() <= packet.cap,
        "{} > {}",
        packet.size(),
        packet.cap
    );
    assert_eq!(packet.cap, 2500);
    let materials = packet.block(PacketBlock::Materials).unwrap();
    assert!(materials.trimmed());
    assert!(
        materials.body().contains("文字省略）"),
        "{}",
        materials.body()
    );
    // 見出しと summary は残る
    assert!(
        materials.body().starts_with("- M0137"),
        "{}",
        materials.body()
    );
    // 決して削らない: current・cast・節のプロット・constraints
    let target = body(&packet, PacketBlock::Target);
    assert!(target.starts_with("current:\n「旧校舎の地下遺跡に眠っているっていう'『星屑の時計』の設計図だ」@アルス\n\n'リリアは溜め息をついたが、図面の精巧な書き込みに目を奪われた。"), "{target}");
    assert!(!body(&packet, PacketBlock::Cast).contains("文字省略"));
    assert!(!body(&packet, PacketBlock::Constraints).contains("文字省略"));
    // 同じ入力なら同じ切り詰め
    assert_eq!(
        packet.text(),
        build(&project, "writer", id(1, 4, 2, 2), &[]).text()
    );
}

#[test]
fn a_packet_that_cannot_fit_is_over_and_cannot_be_sent() {
    let mut project = project();
    let mut settings = project.settings().clone();
    settings.agents.max_chars_per_request = 200;
    project.set_settings(settings);
    let packet = build(&project, "writer", id(1, 4, 2, 2), &[]);
    assert!(packet.over());
    assert!(packet.size() > packet.cap);
}

#[test]
fn max_chars_scales_the_block_budgets_by_the_table_ratio() {
    assert_eq!(budget::table("writer").iter().sum::<usize>(), 7900);
    assert_eq!(budget::table("plotter").iter().sum::<usize>(), 7000);
    assert_eq!(budget::table("stylist").iter().sum::<usize>(), 6600);
    assert_eq!(budget::table("logician").iter().sum::<usize>(), 9500);
    assert_eq!(budget::table("researcher").iter().sum::<usize>(), 5600);
    assert_eq!(budget::table("ideator").iter().sum::<usize>(), 5200);
    assert_eq!(budget::table("integrator").iter().sum::<usize>(), 6000);
    let scaled = budget::budgets("researcher", Some(11200));
    assert_eq!(scaled[6], 6000);
    assert_eq!(budget::cap("writer", None, 12000), 7900);
    assert_eq!(budget::cap("writer", Some(20000), 12000), 12000);
    assert_eq!(budget::cap("writer", None, 0), 7900);
}

#[test]
fn parts_shrink_to_the_budget_and_report_what_was_dropped() {
    let mut block = Block {
        kind: PacketBlock::History,
        budget: 40,
        excluded: false,
        parts: vec![
            Part::new("a", "x".repeat(20), Style::Bare).trimmable(2, 2),
            Part::new("b", "y".repeat(40), Style::Bare).trimmable(2, 1),
        ],
    };
    block.fit();
    assert!(block.size() <= 40, "{}", block.body());
    // seq の小さい b から削る
    assert_eq!(block.parts[0].omitted(), 0);
    assert!(block.parts[1].omitted() > 0);
    assert_eq!(
        block.parts[1].dropped(),
        "y".repeat(block.parts[1].omitted())
    );
    assert!(block.parts[1]
        .content()
        .ends_with(&budget::omitted(block.parts[1].omitted())));
}

#[test]
fn the_ideator_sees_the_existing_ideas_and_the_integrator_sees_recent_tasks() {
    let mut project = project();
    project.insert(Document::new(
        "ideas.md",
        "# I0001 時計塔の鐘\nstatus: inbox\n\n# I0002 地下の湖\nstatus: keep\n",
    ));
    let ideator = build(&project, "ideator", id(1, 4, 2, 2), &[]);
    let materials = body(&ideator, PacketBlock::Materials);
    assert!(
        materials.contains("ideas:\n- I0001 時計塔の鐘\n- I0002 地下の湖"),
        "{materials}"
    );
    for number in 1..=12 {
        let task = Task {
            id: format!("T{number:04}"),
            role: "writer".to_owned(),
            scope: "@01-04-02-02".to_owned(),
            status: "review".to_owned(),
            initiator: "human".to_owned(),
            ..Task::default()
        };
        project.insert(Document::new(
            files::task_path(&task.id),
            files::write_task(&task),
        ));
    }
    let integrator = build(&project, "integrator", id(1, 4, 2, 2), &[]);
    let history = body(&integrator, PacketBlock::History);
    assert!(
        history.starts_with("- T0012 writer @01-04-02-02 review (human)"),
        "{history}"
    );
    assert_eq!(history.lines().count(), 10, "{history}");
    // ほかの役割は同じ節の直近 3 件
    let writer = build(&project, "writer", id(1, 4, 2, 2), &[]);
    assert_eq!(body(&writer, PacketBlock::History).lines().count(), 3);
}

#[test]
fn the_request_puts_the_task_before_the_packet_and_the_system_prompt_apart() {
    let project = project();
    let packet = build(&project, "writer", id(1, 4, 2, 2), &[]);
    let request = packet.request();
    assert!(
        request.system.contains("小説『星屑』の本文担当"),
        "{}",
        request.system
    );
    assert!(
        request
            .user
            .starts_with("節 01-04-02-02 の本文を書いてください。"),
        "{}",
        request.user
    );
    let at = request.user.find("# novnow context").unwrap();
    assert!(at > 0);
    assert_eq!(request.temperature, Some(0.7));
}

#[test]
fn task_examples_are_in_the_packet_and_prompt_variable() {
    let mut project = project();
    project.insert(Document::new(
        "prompts/writer.md",
        "# writer\n## system\nExamples: {{examples}}\n## task\nWrite prose.\n## output\nBody only.\n",
    ));
    let example = vec!["@01-04-02-02 both".to_owned()];
    let packet = packet::build(&Inputs {
        project: &project,
        role: "writer",
        scene: id(1, 4, 2, 3),
        delegation: Delegation::Propose,
        generated_at: NOW,
        title: "星屑",
        excluded: &[],
        task: None,
        comment: "",
        examples: &example,
    });
    assert!(
        packet.examples.contains("@01-04-02-02"),
        "{}",
        packet.examples
    );
    assert!(packet.examples.contains("contents"), "{}", packet.examples);
    assert!(packet.examples.contains("plot"), "{}", packet.examples);
    assert!(packet.request().system.contains("Examples: "));
    assert!(packet.text().contains("## examples"));
    assert!(packet.size() <= packet.cap);
}

#[test]
fn long_examples_are_clipped_consistently_with_prompt_expansion() {
    let mut project = project();
    project.insert(Document::new(
        world::CONTENTS_PATH,
        world::CONTENTS.replace("設計図だ", &"設計図だ".repeat(1000)),
    ));
    project.insert(Document::new(
        "prompts/writer.md",
        "# writer\n## system\nExample: {{examples}}\n## task\nWrite.\n## output\nBody.\n",
    ));
    let mut settings = project.settings().clone();
    settings.agents.max_chars_per_request = 3000;
    project.set_settings(settings);
    let examples = vec!["@01-04-02-02 contents".to_owned()];
    let packet = packet::build(&Inputs {
        project: &project,
        role: "writer",
        scene: id(1, 4, 2, 3),
        delegation: Delegation::Propose,
        generated_at: NOW,
        title: "星屑",
        excluded: &[],
        task: None,
        comment: "",
        examples: &examples,
    });
    assert!(!packet.examples.is_empty());
    assert!(packet.size() <= packet.cap);
    assert!(packet.request().system.contains(&packet.examples));
    assert!(packet.examples.chars().count() < 8000);
}

#[test]
fn chapter_and_episode_plot_examples_are_extracted_at_the_requested_scope() {
    let project = project();
    let specs = ["@01 plot", "@01-04 plot"];
    for spec in specs {
        let examples = vec![spec.to_owned()];
        let packet = packet::build(&Inputs {
            project: &project,
            role: "writer",
            scene: id(1, 4, 2, 3),
            delegation: Delegation::Propose,
            generated_at: NOW,
            title: "星屑",
            excluded: &[],
            task: None,
            comment: "",
            examples: &examples,
        });
        assert!(
            packet.examples.contains(" plot"),
            "{spec}: {}",
            packet.examples
        );
        assert!(
            packet.examples.contains("@01-04"),
            "{spec}: {}",
            packet.examples
        );
        assert!(!packet.examples.contains(" contents"));
    }
}
