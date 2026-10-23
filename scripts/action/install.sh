#!/usr/bin/env bash
# Shared implementation for the composite action. All downloads come from the
# official release and every cached or downloaded archive is checked each run.
set -euo pipefail

MODE="${1:-}"
case "$MODE" in
  --resolve|--install|--run) ;;
  *) echo "usage: install.sh --resolve|--install|--run" >&2; exit 2 ;;
esac

RELEASE_ROOT="https://github.com/sonneko/novnow/releases/download"
TARGETS="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc"

output() {
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    printf '%s=%s\n' "$1" "$2" >> "$GITHUB_OUTPUT"
  else
    printf '%s=%s\n' "$1" "$2"
  fi
}

validate_version() {
  local version="${1#v}"
  if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
    echo "invalid novnow version: $version" >&2
    return 2
  fi
  printf '%s' "$version"
}

validate_target() {
  local target="$1"
  case " $TARGETS " in
    *" $target "*) printf '%s' "$target" ;;
    *) echo "unsupported novnow target: $target" >&2; return 2 ;;
  esac
}

detect_target() {
  if [ -n "${INPUT_TARGET:-}" ]; then
    validate_target "$INPUT_TARGET"
    return
  fi
  local os="${RUNNER_OS:-}" arch="${RUNNER_ARCH:-}"
  case "$os/$arch" in
    Linux/X64|Linux/x64) printf '%s' x86_64-unknown-linux-gnu ;;
    Linux/ARM64|Linux/arm64) printf '%s' aarch64-unknown-linux-gnu ;;
    macOS/X64|macOS/x64) printf '%s' x86_64-apple-darwin ;;
    macOS/ARM64|macOS/arm64) printf '%s' aarch64-apple-darwin ;;
    Windows/X64|Windows/x64) printf '%s' x86_64-pc-windows-msvc ;;
    *) echo "unsupported runner: $os/$arch (set the target input explicitly)" >&2; return 2 ;;
  esac
}

resolve_version() {
  local selected="${INPUT_VERSION:-}"
  if [ -z "$selected" ]; then
    selected="${NOVNOW_ACTION_VERSION:-}"
  fi
  if [ -z "$selected" ] && [ -n "${GITHUB_ACTION_REF:-}" ]; then
    selected="${GITHUB_ACTION_REF##*/}"
  fi
  if [ -z "$selected" ]; then
    echo "cannot determine the action version" >&2
    return 2
  fi
  validate_version "$selected"
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "sha256sum or shasum is required" >&2
    return 2
  fi
}

fetch() {
  local url="$1" destination="$2"
  if [ -f "$destination" ]; then
    return
  fi
  mkdir -p "$(dirname "$destination")"
  local partial="${destination}.partial.$$"
  rm -f "$partial"
  if ! curl --proto '=https' --tlsv1.2 -fsSL --retry 3 "$url" -o "$partial"; then
    rm -f "$partial"
    return 1
  fi
  mv "$partial" "$destination"
}

install_release() {
  local version target artifact extension archive sums expected actual work executable temporary
  version="$(validate_version "${NOVNOW_VERSION:?NOVNOW_VERSION is required}")"
  target="$(validate_target "${NOVNOW_TARGET:?NOVNOW_TARGET is required}")"
  extension=".tar.gz"
  executable="novnow"
  if [ "$target" = x86_64-pc-windows-msvc ]; then
    extension=".zip"
    executable="novnow.exe"
  fi
  artifact="novnow-${version}-${target}-nogui${extension}"
  work="${NOVNOW_CACHE_DIR:-${RUNNER_TEMP:?RUNNER_TEMP or NOVNOW_CACHE_DIR is required}/novnow-bin}/${version}/${target}"
  archive="$work/$artifact"
  sums="$work/SHA256SUMS"
  mkdir -p "$work"

  fetch "$RELEASE_ROOT/v$version/SHA256SUMS" "$sums"
  fetch "$RELEASE_ROOT/v$version/$artifact" "$archive"
  expected="$(awk -v name="$artifact" '!/^#/ && $2 == name { print $1 }' "$sums")"
  if [ -z "$expected" ] || [ "$(printf '%s\n' "$expected" | wc -l | tr -d ' ')" != 1 ]; then
    echo "SHA256SUMS has no unique entry for $artifact" >&2
    return 1
  fi
  actual="$(sha256 "$archive")"
  if [ "$actual" != "$expected" ]; then
    rm -f "$archive" "$work/$executable"
    echo "SHA-256 mismatch for $artifact; the cached/downloaded archive was discarded" >&2
    return 1
  fi

  if [ ! -f "$work/$executable" ]; then
    temporary="$work/.extract.$$"
    rm -rf "$temporary"
    mkdir -p "$temporary"
    while IFS= read -r entry; do
      [ -z "$entry" ] && continue
      case "$entry" in
        /*|../*|*/../*|*/..|[A-Za-z]:/*|*\\*)
          rm -rf "$temporary"
          echo "unsafe archive entry: $entry" >&2
          return 1
          ;;
      esac
    done < <(tar -tf "$archive")
    tar -xf "$archive" -C "$temporary" "$executable"
    if [ ! -f "$temporary/$executable" ]; then
      rm -rf "$temporary"
      echo "$artifact did not contain $executable" >&2
      return 1
    fi
    mv "$temporary/$executable" "$work/$executable"
    rm -rf "$temporary"
  fi
  if [ "$target" != x86_64-pc-windows-msvc ]; then
    chmod +x "$work/$executable"
  fi
  output binary "$work/$executable"
}

finding_count() {
  local json="$1"
  # The canonical CLI emits one JSON envelope. `count` belongs to
  # diagnostics.run; individual diagnostic objects have no `count` field.
  printf '%s' "$json" | sed -n 's/.*"count":\([0-9][0-9]*\).*/\1/p' | head -1
}

run_novnow() {
  local binary="${NOVNOW_BINARY:?NOVNOW_BINARY is required}"
  local command="${INPUT_COMMAND:-check}" path="${INPUT_PATH:-.}"
  local extra=()
  if [ -n "${INPUT_ARGS:-}" ]; then
    # Deliberately no eval and no quote processing (§20.5.2).
    read -r -a extra <<< "$INPUT_ARGS"
  fi

  local code=0 findings="" export_path=""
  set +e
  "$binary" "$command" "$path" "${extra[@]}"
  code=$?
  set -e

  if [ "$command" = check ] && { [ "$code" = 0 ] || [ "$code" = 5 ]; }; then
    local json=""
    if json="$("$binary" diagnostics run "$path" --json 2>/dev/null)"; then
      findings="$(finding_count "$json")"
    fi
  elif [ "$command" = export ] && [ "$code" = 0 ]; then
    local index=0 argument
    for ((index = 0; index < ${#extra[@]}; index += 1)); do
      argument="${extra[$index]}"
      case "$argument" in
        --out)
          if [ $((index + 1)) -lt ${#extra[@]} ]; then
            export_path="${extra[$((index + 1))]}"
          fi
          ;;
        --out=*) export_path="${argument#--out=}" ;;
      esac
    done
  fi

  output exit-code "$code"
  output findings "$findings"
  output export-path "$export_path"
  exit "$code"
}

case "$MODE" in
  --resolve)
    resolved_version="$(resolve_version)"
    resolved_target="$(detect_target)"
    output version "$resolved_version"
    output target "$resolved_target"
    ;;
  --install) install_release ;;
  --run) run_novnow ;;
esac
