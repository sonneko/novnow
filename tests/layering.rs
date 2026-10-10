use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "egui",
    "eframe",
    "rfd",
    "std::fs",
    "web_sys",
    "js_sys",
    "wasm_bindgen",
    "tokio",
    "std::thread",
    "SystemTime::now",
    "Instant::now",
    "thread_rng",
];

fn strip_comments_and_literals(source: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            // Line comment
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            // Block comment
            i += 2;
            let mut depth = 1;
            while i < len && depth > 0 {
                if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if chars[i] == '"' {
            // String literal - replace with empty string
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < len && chars[i] == '"' {
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

fn contains_japanese_string_literals(source: &str) -> Vec<String> {
    let mut japanese_strings = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            let mut depth = 1;
            while i < len && depth > 0 {
                if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if chars[i] == '"' {
            i += 1;
            let mut s = String::new();
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    s.push(chars[i]);
                    s.push(chars[i + 1]);
                    i += 2;
                } else {
                    s.push(chars[i]);
                    i += 1;
                }
            }
            if i < len && chars[i] == '"' {
                i += 1;
            }
            if s.chars().any(|c| {
                matches!(
                    c,
                    '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}'
                )
            }) {
                japanese_strings.push(s);
            }
        } else {
            i += 1;
        }
    }

    japanese_strings
}

fn visit_dirs(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("read_dir failed") {
            let entry = entry.expect("entry failed");
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
}

#[test]
fn test_core_layering_and_no_japanese_literals() {
    let core_dir = Path::new("src/core");
    let mut files = Vec::new();
    visit_dirs(core_dir, &mut files);

    assert!(!files.is_empty(), "No .rs files found in src/core!");

    let mut violations = Vec::new();
    let mut jp_violations = Vec::new();

    for file in files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));

        let stripped = strip_comments_and_literals(&content);

        for &forbidden in FORBIDDEN {
            if stripped.contains(forbidden) {
                violations.push(format!(
                    "Forbidden token '{}' found in {}",
                    forbidden,
                    file.display()
                ));
            }
        }

        let jp_literals = contains_japanese_string_literals(&content);
        for jp_str in jp_literals {
            jp_violations.push(format!(
                "Japanese string literal \"{}\" found in {}",
                jp_str,
                file.display()
            ));
        }
    }

    if !violations.is_empty() || !jp_violations.is_empty() {
        let mut msg = String::new();
        if !violations.is_empty() {
            msg.push_str("Layering violations in src/core:\n");
            for v in violations {
                msg.push_str(&format!("  - {}\n", v));
            }
        }
        if !jp_violations.is_empty() {
            msg.push_str("Japanese literal violations in src/core:\n");
            for v in jp_violations {
                msg.push_str(&format!("  - {}\n", v));
            }
        }
        panic!("{}", msg);
    }
}
