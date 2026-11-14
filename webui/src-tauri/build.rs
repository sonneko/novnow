//! Tauri の組み立て（`tauri.conf.json` と `capabilities/` を読み、権限の表を作る）。

fn main() {
    tauri_build::build()
}
