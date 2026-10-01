//! ストレージ抽象化モジュール（Native: std::fs / WASM: LocalStorage）。

use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const STORAGE_PREFIX: &str = "novnow_file:";
const INDEX_KEY: &str = "novnow_file_index";

#[cfg(target_arch = "wasm32")]
fn get_local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// パスをストレージキー用文字列に正規化する。
pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// テキストファイルを読み込む。
pub fn read_to_string(path: &Path) -> Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(path)
            .with_context(|| format!("ファイルの読み込みに失敗: {}", path.display()))
    }

    #[cfg(target_arch = "wasm32")]
    {
        let storage = get_local_storage().context("LocalStorageを利用できません")?;
        let key = format!("{}{}", STORAGE_PREFIX, normalize_path(path));
        match storage.get_item(&key) {
            Ok(Some(content)) => Ok(content),
            _ => Err(anyhow::anyhow!("ファイルが見つかりません: {}", path.display())),
        }
    }
}

/// テキストファイルを書き込む。
pub fn write_string(path: &Path, content: &str) -> Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, content)
            .with_context(|| format!("ファイルの書き込みに失敗: {}", path.display()))
    }

    #[cfg(target_arch = "wasm32")]
    {
        let storage = get_local_storage().context("LocalStorageを利用できません")?;
        let path_str = normalize_path(path);
        let key = format!("{}{}", STORAGE_PREFIX, path_str);
        storage
            .set_item(&key, content)
            .map_err(|_| anyhow::anyhow!("LocalStorageへの保存に失敗: {}", path.display()))?;

        // ファイルインデックスを更新
        let mut index = get_wasm_file_index(&storage);
        if !index.contains(&path_str) {
            index.push(path_str);
            save_wasm_file_index(&storage, &index);
        }
        Ok(())
    }
}

/// ファイルが存在するかを確認する。
pub fn exists(path: &Path) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        path.exists()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let Some(storage) = get_local_storage() else { return false; };
        let path_str = normalize_path(path);
        let key = format!("{}{}", STORAGE_PREFIX, path_str);
        if storage.get_item(&key).ok().flatten().is_some() {
            return true;
        }
        // ディレクトリ存在チェック（配下にファイルがあるか）
        let dir_prefix = if path_str.ends_with('/') { path_str } else { format!("{path_str}/") };
        let index = get_wasm_file_index(&storage);
        index.iter().any(|f| f.starts_with(&dir_prefix))
    }
}

/// 指定ディレクトリ配下のファイルパス一覧を取得する。
pub fn list_dir(dir_path: &Path) -> Result<Vec<PathBuf>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut paths = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir_path) {
            for entry in rd.flatten() {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let storage = get_local_storage().context("LocalStorageを利用できません")?;
        let dir_str = normalize_path(dir_path);
        let prefix = if dir_str.is_empty() || dir_str == "." {
            String::new()
        } else if dir_str.ends_with('/') {
            dir_str.clone()
        } else {
            format!("{dir_str}/")
        };

        let index = get_wasm_file_index(&storage);
        let mut result = Vec::new();
        for f in index {
            if prefix.is_empty() || f.starts_with(&prefix) {
                // サブディレクトリ直下の直接のエントリのみ抽出する
                let sub = if prefix.is_empty() { &f[..] } else { &f[prefix.len()..] };
                if !sub.is_empty() {
                    result.push(PathBuf::from(&f));
                }
            }
        }
        result.sort();
        Ok(result)
    }
}

/// ファイルを削除する。
pub fn remove_file(path: &Path) -> Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::remove_file(path).with_context(|| format!("削除に失敗: {}", path.display()))
    }

    #[cfg(target_arch = "wasm32")]
    {
        let storage = get_local_storage().context("LocalStorageを利用できません")?;
        let path_str = normalize_path(path);
        let key = format!("{}{}", STORAGE_PREFIX, path_str);
        let _ = storage.remove_item(&key);

        let mut index = get_wasm_file_index(&storage);
        if let Some(pos) = index.iter().position(|x| x == &path_str) {
            index.remove(pos);
            save_wasm_file_index(&storage, &index);
        }
        Ok(())
    }
}

/// WASM環境でストレージが空の場合に初期雛形ファイルを書き込む。
pub fn init_scaffold_if_empty(root: &Path) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(storage) = get_local_storage() else { return; };
        let index = get_wasm_file_index(&storage);
        if !index.is_empty() {
            return;
        }

        let scaffold_files = [
            ("characters.json", "[\n  {\n    \"name\": \"アルス\",\n    \"role\": \"主人公\",\n    \"personality\": \"正義感が強い魔法使い\",\n    \"speech\": \"僕、～だよ\",\n    \"notes\": \"デモプロジェクト\"\n  },\n  {\n    \"name\": \"リリア\",\n    \"role\": \"ヒロイン\",\n    \"personality\": \"冷静沈着な剣士\",\n    \"speech\": \"私、～ね\",\n    \"notes\": \"デモプロジェクト\"\n  }\n]\n"),
            ("words.json", "[\n  {\n    \"term\": \"魔導書\",\n    \"reading\": \"まどうしょ\",\n    \"definition\": \"古代の魔法が記された書物\",\n    \"category\": \"用語\"\n  }\n]\n"),
            ("ideas.json", "[]\n"),
            ("materials/index.json", "[]\n"),
            ("plots/index.json", "[]\n"),
            ("plots/all.txt", "@00-00-00\n;\n"),
            ("plots/preparations.json", "[]\n"),
            ("contents/01-01.txt", "@01-01-01\n「こんにちは、novnowへようこそ！」@アルス\n;\n"),
            ("plots/01-01.txt", "@01-01-01\nアルスとリリアが出会う場面\n;\n"),
            ("project.json", "{\n  \"scene_id_format\": \"A-B-C-D\",\n  \"content_encoding\": \"utf-8\",\n  \"newline\": \"lf\",\n  \"plot_file_extension\": \"txt\"\n}\n"),
        ];

        for (rel, content) in scaffold_files {
            let p = root.join(rel);
            let _ = write_string(&p, content);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = root;
    }
}

/// WASMストレージ内の全ファイルパス一覧を取得する。
#[cfg(target_arch = "wasm32")]
pub fn get_all_stored_files() -> Vec<PathBuf> {
    let Some(storage) = get_local_storage() else { return Vec::new(); };
    get_wasm_file_index(&storage)
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn get_wasm_file_index(storage: &web_sys::Storage) -> Vec<String> {
    storage
        .get_item(INDEX_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn save_wasm_file_index(storage: &web_sys::Storage, index: &[String]) {
    if let Ok(json) = serde_json::to_string(index) {
        let _ = storage.set_item(INDEX_KEY, &json);
    }
}

/// プロジェクトを Zip バッファへエクスポートする。
pub fn export_project_zip(root: &Path) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let files = collect_all_project_files(root)?;
        let root_str = normalize_path(root);

        for path in files {
            if let Ok(content) = read_to_string(&path) {
                let norm = normalize_path(&path);
                let rel_path = if !root_str.is_empty() && norm.starts_with(&format!("{}/", root_str)) {
                    &norm[root_str.len() + 1..]
                } else {
                    &norm
                };

                zip.start_file(rel_path, options)
                    .with_context(|| format!("Zip書き込み失敗: {rel_path}"))?;
                zip.write_all(content.as_bytes())?;
            }
        }
        zip.finish()?;
    }
    Ok(buffer)
}

/// プロジェクト内のすべてのファイルパスを取得する。
fn collect_all_project_files(root: &Path) -> Result<Vec<PathBuf>> {
    #[cfg(target_arch = "wasm32")]
    {
        let root_str = normalize_path(root);
        let prefix = if root_str.is_empty() { String::new() } else { format!("{root_str}/") };
        let all = get_all_stored_files();
        Ok(all.into_iter().filter(|p| {
            let s = normalize_path(p);
            prefix.is_empty() || s.starts_with(&prefix)
        }).collect())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut files = Vec::new();
        fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        walk(&p, files);
                    } else if p.is_file() {
                        files.push(p);
                    }
                }
            }
        }
        walk(root, &mut files);
        Ok(files)
    }
}

/// Zip バッファからプロジェクトをインポート（展開して保存）する。
pub fn import_project_zip(zip_bytes: &[u8], root: &Path) -> Result<()> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Zipアーカイブの読み込みに失敗")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let raw_name = file.name().replace('\\', "/");
        if file.is_dir() || raw_name.contains("__MACOSX") || raw_name.starts_with('.') {
            continue;
        }

        let parts: Vec<&str> = raw_name.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }

        // アーカイブ内が単一ルートディレクトリで囲まれている場合（例: `my-novel/contents/...`）
        let rel_parts = if parts.len() > 1 && (parts[0] == "my-novel" || parts[0].ends_with("-main") || parts[0].ends_with("-master")) {
            &parts[1..]
        } else {
            &parts[..]
        };

        let rel_path = rel_parts.join("/");
        let target_path = root.join(&rel_path);

        let mut content = String::new();
        if file.read_to_string(&mut content).is_ok() {
            write_string(&target_path, &content)?;
        }
    }

    Ok(())
}

/// ブラウザ上でファイルダウンロードをトリガーする。
#[cfg(target_arch = "wasm32")]
pub fn trigger_browser_download(file_name: &str, data: &[u8]) -> Result<()> {
    use wasm_bindgen::JsCast;

    let array = js_sys::Uint8Array::from(data);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array.buffer());

    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/zip");

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&blob_parts, &options)
        .map_err(|_| anyhow::anyhow!("Blobの作成に失敗しました"))?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| anyhow::anyhow!("URLの作成に失敗しました"))?;

    let document = web_sys::window()
        .and_then(|w| w.document())
        .context("documentを取得できません")?;

    let a = document
        .create_element("a")
        .map_err(|_| anyhow::anyhow!("aタグの作成に失敗しました"))?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| anyhow::anyhow!("HtmlElementへのキャストに失敗しました"))?;

    a.set_attribute("href", &url).ok();
    a.set_attribute("download", file_name).ok();
    a.click();

    web_sys::Url::revoke_object_url(&ok_url_ref(&url)).ok();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn ok_url_ref(s: &str) -> String {
    s.to_string()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn trigger_browser_download(_file_name: &str, _data: &[u8]) -> Result<()> {
    Ok(())
}
