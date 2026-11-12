# my-novels 用 nightly 更新スクリプト

`REF:scripts/update-novnow-nightly.py` は単独で動作する。my-novels の `scripts/` にコピーして使う。novnow のビルドや他のソースファイルは不要。

## 必要なもの

- Linux x86_64（x64 の WSL・Linux コンテナを含む）
- Python 3.9 以降
- 認証済みの GitHub CLI `gh`
- VS Code CLI `code`（ダウンロードのみなら不要）

VSIX の導入先は、実行した `code` コマンドが管理する VS Code 環境。WSL・SSH・Dev Container では、実際に novnow を使う側で実行する。特定のプロファイルには `--profile` を指定する。更新前に原稿を保存する。

## 使用方法

my-novels のルートで実行する。

```bash
python3 scripts/update-novnow-nightly.py
```

初回のみ、出力された設定を既存の VS Code ワークスペース設定へ追加する。

```json
{
  "novnow.executablePath": "/absolute/path/to/my-novels/.tools/novnow/current/novnow",
  "novnow.autoDownload": false
}
```

スクリプトは既存の設定ファイルを上書きしない。絶対パスは実際の出力を使う。`.tools/novnow/` は my-novels 側の `.gitignore` に追加する。更新後に VS Code の「Developer: Reload Window」を実行し、拡張と LSP を再起動する。

CLI は次の固定パスで呼び出せる。

```bash
.tools/novnow/current/novnow --version
.tools/novnow/current/novnow check .
```

保存先や VS Code を変更する場合:

```bash
python3 scripts/update-novnow-nightly.py --install-dir "$HOME/.local/share/my-novels/novnow"
python3 scripts/update-novnow-nightly.py --code code-insiders --profile "Novel"
```

導入せず、ダウンロードと検証だけ行う場合:

```bash
python3 scripts/update-novnow-nightly.py --download-only
```

このモードはバイナリを実行せず、VSIX も導入せず、`current` も変更しない。生成先を標準出力に表示する。

## 更新と復旧

- 正式版ではなく `sonneko/novnow` の `nightly` を取得する。ブラウザ版は取得しない。
- Release の asset ID を固定して取得し、バイナリ・VSIX・manifest の SHA-256 を確認する。取得中に nightly の公開世代が変わったら、組全体を取り直す。最大 3 回失敗すると、現在の選択を変えず終了する。
- 検証後にバイナリの起動と製品版番号を確認し、VSIX を `--force` で導入する。同じ版番号の新しい nightly も対象にする。VSIX 成功後に `current` リンクを切り替え、直前の選択を `previous` に残す。
- SHA は出典の識別に使い、チェックサムは取得した配布物との整合検証に使う。独立した署名認証ではない。
- VS Code の拡張導入とリンク更新を完全に一括処理することはできない。導入失敗時や処理中断時は VS Code 側も確認する。検証済みの旧 VSIX とバイナリは削除しない。

前回版へ戻す場合は、VS Code を閉じ、my-novels のルートで次を実行する。独自保存先・プロファイルを指定した場合は同じ設定に読み替える。更新スクリプトと同時に実行しない。

```bash
code --install-extension .tools/novnow/previous/novnow-nightly.vsix --force && \
python3 - <<'PY'
import os
from pathlib import Path
root = Path('.tools/novnow')
previous = root / 'previous'
assert previous.is_symlink() and previous.resolve().is_dir(), 'previous is unavailable'
link = root / '.rollback'
link.symlink_to(os.readlink(previous))
os.replace(link, root / 'current')
PY
```

保存した世代は自動削除しない。不要な世代を消す場合は `current` と `previous` の参照先を残す。VSCode を再度開いて使用する。
