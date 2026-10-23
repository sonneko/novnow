## 概要

<!-- 何を変えたか。作業項目単位で（`docs/plans.md` の該当節 / `docs/history.md` の ID を添えて） -->

## 参照した仕様

<!-- 例: `docs/specifications/syntax.md` §5.1、`docs/plans.md` §4.1 の追加仕様 12 -->

- [ ] `docs/specifications/**.md` / `docs/designs/**.md` の該当章を参照した（上に §番号を書いた）

## チェックリスト

- [ ] `docs/plans.md` §4.4 の縫い代チェック（① の実装が ② の追加仕様を妨げていないか）
- [ ] `tests/layering.rs` が通る（CI の Test ジョブが緑）
- [ ] `docs/plans.md` §4.2 の表にある ②（追加仕様）の範囲からはみ出していない
- [ ] 仕様からの逸脱・設計判断は `docs/history.md` に ID（`[HIST:...]`）付きで記録した
- [ ] `bash scripts/verify.sh` が緑（`docs/developers.md` §4.1）
