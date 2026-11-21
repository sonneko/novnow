// 画面が持つ状態（**セッションの状態を写したものだけ**。§11.11.5）。
//
// `src/**`（機能核）を写しにいくのではなく、正規操作の戻り値と `poll_events` のイベント
// だけを材料にする。ワークスペースはこの状態を読むだけで、独自のドメイン規則を持たない。
//
// 型の大半は `bridge/types.ts` から来る（凍結）。ここは「画面が持ち回る形」だけを決める。

import type { Bridge, OpId, OpArgs, OpResult, SaveState } from '../bridge/types';

/** 節 1 つ（`section.list` の要素。`REF:src/core/features/sections.rs` の `SectionRef` の写し）。 */
export interface SectionRef {
  /** `01-04-02-03` の 4 階層 ID。 */
  id: string;
  /** 文書のパス。 */
  path: string;
  /** 文書内の行範囲（1 始まり）。 */
  lines: [number, number];
  /** 見出し行（`@…` の行）。 */
  header: string;
  /** 場面の題名（`header` から `@…` を除いたもの）。 */
  scene_title: string;
}

/** 画面上部に出す 1 行（エラー・通知）。 */
export interface Notice {
  /** 種別（`info` / `error`）。 */
  kind: 'info' | 'error';
  /** 表示する文。 */
  text: string;
  /** 消すまでのミリ秒（0 なら手で消すまで残す）。 */
  ttl: number;
}

/** 画面が持つ状態。 */
export interface Screen {
  /** 開けているか。 */
  ready: boolean;
  /** 開けなかった理由。 */
  failure: string;
  /** プロジェクトの題名。 */
  title: string;
  /** プロジェクトの ID（保存領域のキー）。 */
  projectId: string;
  /** 版。 */
  version: string;
  /** いまの言語（`ja` / `en`）。 */
  locale: string;
  /** いまのワークスペース（`write` だけ実装）。 */
  workspace: string;
  /** 使えるワークスペースの一覧（§10 の 9 つ）。 */
  workspaces: string[];
  /** いまの節（`01-04-02-03`。空なら未選択）。 */
  scene: string;
  /** パンくず（`01›04›02›03`）。 */
  breadcrumb: string;
  /** いまの節の文書のパス。 */
  path: string;
  /** いまの節の本文。 */
  text: string;
  /** 節の一覧（`section.list`）。 */
  sections: SectionRef[];
  /** 保存状態機の 6 状態（§9.6）。 */
  save: SaveState;
  /** 保存の失敗の理由（`save` が `error` のとき）。 */
  saveMessage: string;
  /** 未保存のパス（`save.state`）。 */
  unsaved: string[];
  /** プロジェクト全体の文字数（`project.info` の `chars`）。 */
  projectChars: number;
  /** 話の文字数（いまの文書の節の合計）。 */
  documentChars: number;
  /** 節の文字数（いまの節）。 */
  sectionChars: number;
  /** 診断の件数（エラー + 警告。`project.validate`）。 */
  diagnostics: number;
  /** 参照ペインの材料（`reference.pane` の戻り値そのまま）。 */
  reference: Record<string, unknown> | null;
  /** 参照ペインを出すか（`Ctrl+\`。§9.8.1）。 */
  referenceVisible: boolean;
  /** 通知。 */
  notices: Notice[];
  /** `flush` が未完了の書き込みを残したか（保存領域の失敗）。 */
  pending: number;
}

/** 初期状態。 */
export function initial(): Screen {
  return {
    ready: false,
    failure: '',
    title: '',
    projectId: '',
    version: '',
    locale: 'ja',
    workspace: 'write',
    workspaces: [
      'write',
      'plot-episode',
      'plot-outline',
      'idea',
      'material',
      'cast',
      'reader',
      'agent',
      'review',
    ],
    scene: '',
    breadcrumb: '',
    path: '',
    text: '',
    sections: [],
    save: 'idle',
    saveMessage: '',
    unsaved: [],
    projectChars: 0,
    documentChars: 0,
    sectionChars: 0,
    diagnostics: 0,
    reference: null,
    referenceVisible: true,
    notices: [],
    pending: 0,
  };
}

/** 操作 1 回の結果（`ok: false` なら通知にして返す）。 */
export type Outcome<T extends OpId> =
  | { ok: true; result: OpResult[T] }
  | { ok: false; code: string; message: string };

/** セッションの口を包んだもの（画面のどこからでも同じ手順で呼ぶため）。 */
export class Session {
  /** 状態（Svelte 5 の rune。`$state` の代わりに素のオブジェクトを差し替える）。 */
  screen: Screen = initial();
  /** 状態が変わったら呼ばれる（`main.ts` が Svelte の store へ流す）。 */
  private readonly listeners = new Set<(screen: Screen) => void>();

  constructor(private readonly bridge: Bridge) {}

  /** 状態を見る（読み取り専用）。 */
  get state(): Screen {
    return this.screen;
  }

  /** 状態の変化を購読する。 */
  subscribe(found: (screen: Screen) => void): () => void {
    this.listeners.add(found);
    found(this.screen);
    return () => {
      this.listeners.delete(found);
    };
  }

  /** 状態を書き換えて通知する。 */
  update(change: (screen: Screen) => void): void {
    const next = { ...this.screen };
    change(next);
    this.screen = next;
    for (const found of this.listeners) {
      found(next);
    }
  }

  /** 正規操作 1 回。引数の無い操作でも `args` を省ける。 */
  async run<T extends OpId>(op: T, args?: OpArgs[T]): Promise<Outcome<T>> {
    const found = await this.bridge.run(op, args as Record<string, unknown> | undefined);
    if (!found.ok) {
      return { ok: false, code: found.error.code, message: found.error.message };
    }
    return { ok: true, result: found.result as OpResult[T] };
  }

  /** 失敗を通知にする（画面の操作から呼ぶ）。 */
  notice(text: string, kind: Notice['kind'] = 'info', ttl = 4000): void {
    this.update((screen) => {
      screen.notices = [...screen.notices, { kind, text, ttl }];
    });
  }

  /** 通知を消す。 */
  dismiss(index: number): void {
    this.update((screen) => {
      screen.notices = screen.notices.filter((_, at) => at !== index);
    });
  }
}
