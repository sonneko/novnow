use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{ExprPath, ItemUse, LitStr, UseTree};

const FORBIDDEN_PREFIXES: &[&[&str]] = &[
    &["egui"],
    &["eframe"],
    &["rfd"],
    &["std", "fs"],
    &["std", "net"],
    &["std", "process"],
    &["web_sys"],
    &["js_sys"],
    &["wasm_bindgen"],
    &["tokio"],
    &["std", "thread"],
    &["fontdb"],
    &["zip"],
    &["toml"],
    &["directories"],
    &["crate", "ui"],
    &["crate", "adapters"],
    &["ui"],
    &["adapters"],
];

struct LayerVisitor {
    filepath: PathBuf,
    violations: Vec<String>,
    jp_violations: Vec<String>,
}

impl LayerVisitor {
    fn check_path_segments(&mut self, segments: &[String], context: &str) {
        for forbidden in FORBIDDEN_PREFIXES {
            if segments.len() >= forbidden.len() {
                let matches = forbidden.iter().zip(segments.iter()).all(|(f, s)| f == s);
                if matches {
                    let path_str = segments.join("::");
                    self.violations.push(format!(
                        "Forbidden import/access '{}' in {} ({})",
                        path_str,
                        self.filepath.display(),
                        context
                    ));
                }
            }
        }

        // Specific forbidden calls/paths
        let full_path = segments.join("::");
        if full_path.contains("SystemTime::now")
            || full_path.contains("Instant::now")
            || full_path.contains("thread_rng")
        {
            self.violations.push(format!(
                "Forbidden time/random call '{}' in {} ({})",
                full_path,
                self.filepath.display(),
                context
            ));
        }
    }

    fn check_use_tree(&mut self, tree: &UseTree, prefix: &mut Vec<String>) {
        match tree {
            UseTree::Path(use_path) => {
                prefix.push(use_path.ident.to_string());
                self.check_use_tree(&use_path.tree, prefix);
                prefix.pop();
            }
            UseTree::Name(use_name) => {
                prefix.push(use_name.ident.to_string());
                self.check_path_segments(prefix, "use statement");
                prefix.pop();
            }
            UseTree::Rename(use_rename) => {
                prefix.push(use_rename.ident.to_string());
                self.check_path_segments(prefix, "use statement");
                prefix.pop();
            }
            UseTree::Glob(_) => {
                self.check_path_segments(prefix, "use glob statement");
            }
            UseTree::Group(use_group) => {
                for item in &use_group.items {
                    self.check_use_tree(item, prefix);
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for LayerVisitor {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut prefix = Vec::new();
        self.check_use_tree(&node.tree, &mut prefix);
        visit::visit_item_use(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast ExprPath) {
        let segments: Vec<String> = node
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        self.check_path_segments(&segments, "expr path");
        visit::visit_expr_path(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast LitStr) {
        let s = node.value();
        if s.chars().any(|c| {
            matches!(
                c,
                '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}'
            )
        }) {
            self.jp_violations.push(format!(
                "Japanese string literal \"{}\" in {}",
                s,
                self.filepath.display()
            ));
        }
        visit::visit_lit_str(self, node);
    }
}

fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) {
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

        let syntax = syn::parse_file(&content)
            .unwrap_or_else(|e| panic!("Failed to parse Rust AST in {}: {}", file.display(), e));

        let mut visitor = LayerVisitor {
            filepath: file.clone(),
            violations: Vec::new(),
            jp_violations: Vec::new(),
        };

        visitor.visit_file(&syntax);

        violations.extend(visitor.violations);
        jp_violations.extend(visitor.jp_violations);
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
