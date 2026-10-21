//! Decode only bounded PNG/JPEG project assets. Never creates network image sources.
use std::hash::{Hash, Hasher};
pub fn show(ui: &mut egui::Ui, bytes: &[u8], size: f32) {
    if bytes.len() > 256 * 1024 {
        return;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hash);
    let id = egui::Id::new(("portrait", hash.finish()));
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<Option<egui::TextureHandle>>(id));
    let texture = cached.unwrap_or_else(|| {
        let reader = image::io::Reader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        if !matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ) {
            return None;
        }
        let (width, height) = reader.into_dimensions().ok()?;
        if width as u64 * height as u64 > 16_000_000 {
            return None;
        }
        let image = image::load_from_memory(bytes)
            .ok()?
            .thumbnail(256, 256)
            .to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        Some(
            ui.ctx()
                .load_texture(format!("{id:?}"), color, egui::TextureOptions::LINEAR),
        )
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(id, texture.clone()));
    if let Some(texture) = texture {
        ui.image((texture.id(), egui::Vec2::splat(size)));
    }
}
pub fn character(
    ui: &mut egui::Ui,
    project: &crate::core::features::Project,
    name: &str,
    size: f32,
) {
    let parsed = project.characters();
    if let Some((index, _)) = parsed
        .characters
        .iter()
        .enumerate()
        .find(|(_, c)| c.name == name)
    {
        if let Some(field) = parsed
            .character_fields(index, crate::core::text::lexer::FieldKey::Icon)
            .first()
        {
            if let Some(path) = crate::core::stage_two::metadata::asset_path(field.value, true) {
                if let Some(bytes) = project.assets.get(&path) {
                    show(ui, bytes, size);
                }
            }
        }
    }
}
