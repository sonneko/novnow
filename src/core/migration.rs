//! Plain-text migration: deterministic conversion without I/O or source paths.
use crate::core::template::{self, TemplateFile};
use crate::core::text::lexer;
use crate::core::text::lexer::Newline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationError {
    Empty,
    Nul,
}

/// Create a single episode, scene and section, without inferring structure.
pub fn convert(title: &str, source: &str) -> Result<Vec<TemplateFile>, MigrationError> {
    if source.contains('\0') {
        return Err(MigrationError::Nul);
    }
    let source = lexer::normalize_for_save(source, Newline::Lf);
    if source.trim().is_empty() {
        return Err(MigrationError::Empty);
    }
    let mut contents = String::from("@01-01-01\n");
    for ch in source.chars() {
        if lexer::ESCAPE_TARGETS.contains(&ch) {
            contents.push('\\');
        }
        contents.push(ch);
    }
    let mut files = template::new_project(title);
    for file in &mut files {
        file.text = match file.path {
            "contents/01-01.cnov" => contents.clone(),
            "plots/all.cnov" => "@01\nsummary: \n".to_owned(),
            "plots/01.cnov" => "@01-01\nsummary: \n".to_owned(),
            "plots/01-01.cnov" => "@01-01-01\n".to_owned(),
            "characters.md" | "words.md" | "ideas.md" => String::new(),
            _ => continue,
        };
    }
    Ok(files)
}
