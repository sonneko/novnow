# nightly-webui runtime regression report

The browser runs used the production build from this checkout and headless Chromium at `/novnow/nightly-webui/`.
Full event/request data is retained in `nightly-webui-browser-before.json` and `nightly-webui-browser.json`; the runner is `nightly-webui-browser.mjs`.

## Before fix

- All 9 workspace tabs switched successfully; the body edit reached IndexedDB and survived reload; save state was `clean`.
- Reader switch raised the following page error twice (verbatim):

  ```text
  https://svelte.dev/e/each_key_duplicate
  ```

- Console messages: 0. Unhandled Promise rejections: 0. Failed requests: 0.
- The active nightly URL incorrectly opened `novnow-webui`.
- With `data-state="clean"`, the save button accessible name was `保存中`.
- The original event stack and network records are preserved in `nightly-webui-browser-before.json`.

## After fix

- All 9 workspace tabs switched successfully, including Reader; no page errors.
- Console messages: 0. Unhandled Promise rejections: 0. Failed requests: 0.
- The nightly URL opened `novnow-webui-nightly`; the write and post-reload read were both found in that DB.
- Save state transitioned `idle` → `clean`, remained `clean` after the 30-second autosave wait, and remained `clean` after reload. The clean/idle accessible name is `保存`.
- Edited probe text remained in `contents/01-04.cnov` after reload.

## Test-first record

Before production changes, the three new regression checks failed as intended:

- `npm test`: 192 passed, 2 failed (duplicate-key regression and clean/idle save-name regression).
- `cargo test --manifest-path crates/novnow-wasm/Cargo.toml db_name_follows_the_entry_point -- --exact storage::tests::db_name_follows_the_entry_point`: failed because `/novnow/nightly-webui/` returned `novnow-webui` instead of `novnow-webui-nightly`.

After the changes:

- `bash scripts/webui-check.sh --full`: passed; generated assets matched, wasm check passed, `svelte-check` reported 0 errors / 0 warnings, all 194 Node tests passed, and Vite production build passed.
- `cargo test --manifest-path crates/novnow-wasm/Cargo.toml`: all 6 tests passed (including stable/nightly DB-name separation).
- `git diff --check`: passed.
- The post-fix browser run completed with the outcomes above.

The hosted CI webui job will be checked on the PR before completion.
