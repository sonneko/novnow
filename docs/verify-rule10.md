# 規則 10 の確認手順 — プロジェクト外に作られるファイルは履歴 1 個（と、保存ダイアログで指定した Zip）だけ

> 対象: `SPECIFICATIONS.md` §13.1（規則 10）・§19.10、`PLANS.md` §5.6 の完了条件 2。
> 最終更新: 2026-10-13（M6。§13.1.1 の特例＝仕様 第 2.10 版を反映）

> 「プロジェクトディレクトリ外に存在して良い情報は、プロジェクトディレクトリパスの履歴のみ。
> これは厳密に守る必要がある。」（開発指示 10）

## 1. 合格条件

novnow を一通り操作したあと、**プロジェクトの外に新しく作られた（または書き換えられた）ファイルが
パス履歴ファイル 1 個と、保存ダイアログで指定した Zip だけ**であること。履歴ファイルの中身は
`schema_version` と `items[].path` / `items[].opened_at` だけであること（§13.2.2・§19.10）。

**特例**（§13.1.1、ユーザー裁定 2026-10-13・仕様 第 2.10 版）: Zip のエクスポートで、保存ダイアログで
プロジェクトの外の `.zip` を指定したときだけ、その 1 個が外に作られる（上書きされる）。そのとき
**一時ファイルも親ディレクトリも作られない**こと。ダイアログでプロジェクトの外を指定しなかった回は、
外に作られるのは履歴だけ。

| OS | 履歴ファイル（§13.2.1。`NOVNOW_RECENT_FILE` で上書きできる） |
|---|---|
| Linux / BSD | `$XDG_STATE_HOME/novnow/recent.json`（未設定なら `~/.local/state/novnow/recent.json`） |
| macOS | `~/Library/Application Support/novnow/recent.json` |
| Windows | `%APPDATA%\novnow\recent.json` |

履歴ファイルの親ディレクトリ（`novnow/` など）が作られるのは合格の範囲（§13.2.2 第 2.9 版）。
原子的保存の一時ファイル（`.<name>.novnow-tmp`）はプロジェクト内に作られ、保存が終われば消える。

## 2. 自動の検査（CI で毎回）

| テスト | 確かめること |
|---|---|
| `tests/storage_policy.rs` | **全書き込み経路**（本文・設定・エクスポート・提案・ログ・Zip・履歴）にプロジェクト外のパス（絶対パス・`..`・外を指すシンボリックリンク・壊れたリンク・閉じたプロジェクト・前のプロジェクト）を与えて、すべて拒否され何も書かれない。特例は `the_save_dialog_exception_*`（指定した `.zip` 1 個だけ・`.zip` でないパスと履歴は拒否・他の口は使えない・親ディレクトリも一時ファイルも作らない・リンクは行き先で判定）。`after_a_full_session_the_only_files_outside_the_projects_are_the_history_and_the_chosen_zip` はストレージ層で一通りの操作をなぞり、砂場全体の差分が「プロジェクト 2 つ ＋ 履歴 1 個 ＋ 指定した Zip 1 個」だけであることを見る |
| `tests/layering.rs` | 書き込みの system call（`OpenOptions`・`fs::write`・`fs::rename` 等）が `src/adapters/storage_fs.rs`（書き込みの関門）にしか無い（§13.1.2） |
| `tests/recent_file.rs` | 履歴ファイルのキーが 2 つだけ・一時ファイルを作らない |
| `tests/cli.rs` | `novnow check` / `novnow <dir>`（不備）/ `novnow new` が、プロジェクトの外に何も作らない（履歴も書かない、§14.2.3） |
| `tests/storage_idb.rs` | Web の `app` ストアのキーが `recent_projects` だけ（§13.3.4） |

ここまでは**ストレージ層の約束**。GUI のライブラリ（下の 4.）が勝手に書くものは自動の検査では
捕まらないので、実機の確認（3.）が要る。

## 3. 実機での確認

`<project>` は確認に使うプロジェクト、`<build>` は novnow の実行ファイル。**履歴ファイルを消してから
始める**と、作られたかどうかが分かりやすい。

### 3.1 操作の一覧（毎回すべて行う）

M6 では CLI（`novnow new` / `novnow check` / `novnow <project>`）だけ。M7 以降は GUI で次を一通り行う。

1. 起動 → プロジェクト選択（履歴から / フォルダを開く）
2. 本文を編集して自動保存させる（節の移動・`Ctrl+S`・ウィンドウの非フォーカス）
3. ワークスペースを全部切り替える（`ui.json` が書かれる）
4. テキストのエクスポート（`exports/`）と Zip のエクスポート（保存ダイアログで 1 回は初期値の
   `exports/<name>.zip`、もう 1 回は**プロジェクトの外**を指定する。外に増えてよいのはその Zip 1 個だけ）
5. Zip のインポート（新しいディレクトリへ）
6. 提案（`agents/proposals/`）・エージェントのログ（`agents/log/`。M10 以降）
7. 外部のエディタで本文を書き換えて、再読込（I052）を起こす
8. 終了（保存中に閉じる → 保存完了後に閉じる）

### 3.2 Linux: ファイル差分（いちばん手軽）

```bash
touch /tmp/novnow-rule10.marker && sleep 1
<build> <project>            # 3.1 の操作をして終了する
find / -xdev \( -path /proc -o -path /sys -o -path /dev \) -prune -o \
     -newer /tmp/novnow-rule10.marker -type f -print 2>/dev/null \
  | grep -v "^$(realpath <project>)/" | sort
```

出てきたものから、novnow と無関係なもの（シェルの履歴・システムのログ・同時に動いていた他のプロセスの
書き込み）を除いて、**履歴ファイルと、保存ダイアログで指定した Zip の行だけ**なら合格。

### 3.3 Linux: strace（書き込みの system call を全部見る）

```bash
strace -f -e trace=open,openat,openat2,creat,mkdir,mkdirat,rename,renameat,renameat2,unlink,unlinkat,link,linkat,symlink,symlinkat,truncate \
       -o /tmp/novnow.strace <build> <project>
# 書き込み（O_WRONLY / O_RDWR / O_CREAT）と、作成・改名・削除だけを取り出す
grep -E 'O_WRONLY|O_RDWR|O_CREAT|mkdir|rename|unlink|link|truncate' /tmp/novnow.strace \
  | grep -v ' = -1 ' | grep -oE '"[^"]+"' | tr -d '"' | sort -u \
  | grep -v "^$(realpath <project>)/"
```

`/dev/` や `/proc/self/` のような疑似ファイルを除いて、履歴ファイル（とその親ディレクトリ）と、保存ダイアログで
指定した Zip だけなら合格（その Zip の隣に一時ファイルや新しいディレクトリが出たら不合格）。

### 3.4 macOS

```bash
sudo fs_usage -w -f filesys | grep -i novnow > /tmp/novnow.fs_usage   # 別の端末で
```

`open`（書き込みモード）/ `WrData` / `rename` / `mkdir` の行からプロジェクト外のパスを拾う。
`~/Library/Application Support/novnow/recent.json` と保存ダイアログで指定した Zip 以外があれば不合格。

### 3.5 Windows

Process Monitor（Sysinternals）で `Process Name is novnow.exe` に絞り、`Operation` が
`CreateFile`（Disposition が Create / OpenIf / OverwriteIf）・`WriteFile`・`SetRenameInformationFile`
の行を見る。`%APPDATA%\novnow\recent.json` と保存ダイアログで指定した Zip 以外のプロジェクト外のパスがあれば不合格。

## 4. GUI で注意する書き込み（M7 で必ず確かめる）

novnow のコードは関門（`FsGate`）を通らないと書けないが、**GUI のライブラリが自分で書く**ものは
関門の外にある。M7 で次を確かめ、出たら止める（止められないものは仕様側の扱いをユーザーに相談する）。

| 出どころ | 書くもの | 対策の候補 |
|---|---|---|
| eframe の `persistence` feature | アプリの状態（`~/.local/share/<app>/` 等） | **feature を有効にしない**（`ui.json` が担う、§13.1.1） |
| GTK のファイル選択（rfd の GTK 実装） | `~/.local/share/recently-used.xbel`（最近使ったファイル。**保存ダイアログでも書かれうる**） | xdg-desktop-portal 実装を使う・GTK の recent を無効にする |
| Mesa / GPU ドライバ | シェーダのキャッシュ（`~/.cache/mesa_shader_cache` 等） | 描画バックエンドの選択・環境変数（要相談） |
| fontconfig | フォントのキャッシュ（`~/.cache/fontconfig`） | システムのキャッシュを使うだけなら書かれない（初回だけ書かれることがある） |

## 5. 実施記録

| 日付 | 版 | OS | 方法 | 結果 |
|---|---|---|---|---|
| 2026-10-13 | M5（ストレージ層のみ。CLI は M6・GUI は M7） | Linux（Debian 12） | 3.2 のファイル差分。`tests/storage_policy.rs` の `after_a_full_session_the_only_file_outside_the_project_is_the_history`（開く・読む・保存・`ui.json`・ログの追記・提案・Zip のエクスポート・削除・別ディレクトリへの Zip の取り込み・履歴 2 回）を `TMPDIR` と `HOME` を空のディレクトリに向け、`NOVNOW_TEST_KEEP=1`（砂場を残す）で実行 | ファイルシステム全体で新しいファイルは 2 つのプロジェクトの中身と `home/.local/state/novnow/recent.json` だけ（ディレクトリは `.local/state/novnow/` までの親だけ）。**合格**（ストレージ層の範囲） |
| 2026-10-13 | M6（CLI の実行ファイル。GUI は M7） | Linux（Debian 12） | 3.2 のファイル差分。`HOME` と `TMPDIR` を空のディレクトリに向け、`NOVNOW_RECENT_FILE` / `XDG_STATE_HOME` を外して（＝ §13.2.1 の既定の場所）、`novnow new novel`・`check novel`・`novnow novel`（検査を通って GUI なしで 2）・`export novel`（枠）・`novnow empty`（不備で 3）・`new empty`（既存のディレクトリに雛形）・`check empty`・`migrate novel`・`--help` を実行 | 新しいファイルは 2 つのプロジェクトの中だけ。`HOME` も `TMPDIR` も空のまま（M6 は履歴にも書かない、§14.2.3・B-105）。ディレクトリの変化はプロジェクトを作った親（作業ディレクトリ）の更新時刻だけ。**合格**（CLI の範囲） |
