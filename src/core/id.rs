// SceneId, PartialSceneId, RangeExpr

// ---- PLANS.md §5.1 完了条件2 の証跡採取用（次のコミットで削除する）----
// 層検査が「実際に違反を落とす」ことを CI 上で示すための一時的なコード。
// 型位置の依存（旧検査器が見逃していた形）とユーザー向け日本語リテラルを
// 1 つずつ混ぜてある。コンパイルは通り、warning も出ない。
pub fn probe(ctx: &egui::Context) -> &egui::Context {
    ctx
}

pub const PROBE_TEXT: &str = "層検査の証跡";
