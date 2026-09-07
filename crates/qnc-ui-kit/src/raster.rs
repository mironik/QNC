use eframe::egui::{self, Color32, Rect, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::HashMap;

#[derive(Clone)]
struct Texture {
    handle: TextureHandle,
    used: u64,
}
#[derive(Clone, Default)]
struct RasterCache {
    clock: u64,
    textures: HashMap<(String, u64), Texture>,
}

/// Passive upload/paint of already decoded pixels. No URI fetching or decoding.
pub fn paint_rgba_image(
    ui: &mut Ui,
    rect: Rect,
    uri: &str,
    content_key: u64,
    size: [usize; 2],
    rgba: &[u8],
) -> bool {
    if !ui.is_rect_visible(rect) {
        return false;
    }
    if size.contains(&0)
        || size[0].checked_mul(size[1]).and_then(|v| v.checked_mul(4)) != Some(rgba.len())
        || rgba.len() > 4 * 1024 * 1024
    {
        return false;
    }
    let key = (uri.to_string(), content_key);
    let cached = ui.data_mut(|data| {
        let cache =
            data.get_temp_mut_or_default::<RasterCache>(egui::Id::new("qnc-ui-kit-raster-cache"));
        cache.clock = cache.clock.wrapping_add(1);
        cache.textures.get_mut(&key).map(|entry| {
            entry.used = cache.clock;
            entry.handle.clone()
        })
    });
    // Upload outside the egui memory lock: load_texture also accesses the context.
    let texture = cached.unwrap_or_else(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba);
        let handle = ui.ctx().load_texture(
            format!("{uri}:{content_key}"),
            image,
            TextureOptions::LINEAR,
        );
        ui.data_mut(|data| {
            let cache = data
                .get_temp_mut_or_default::<RasterCache>(egui::Id::new("qnc-ui-kit-raster-cache"));
            if cache.textures.len() >= 128 {
                if let Some(oldest) = cache
                    .textures
                    .iter()
                    .min_by_key(|(_, t)| t.used)
                    .map(|(k, _)| k.clone())
                {
                    cache.textures.remove(&oldest);
                }
            }
            cache.textures.insert(
                key,
                Texture {
                    handle: handle.clone(),
                    used: cache.clock,
                },
            );
        });
        handle
    });
    let aspect = size[0] as f32 / size[1] as f32;
    let width = rect.width().min(rect.height() * aspect);
    let fitted = Rect::from_center_size(rect.center(), Vec2::new(width, width / aspect));
    ui.painter().image(
        texture.id(),
        fitted,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paints_supplied_pixels_without_resource_fetching() {
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(160.0, 90.0));
                assert!(paint_rgba_image(
                    ui,
                    rect,
                    "qnc://local/artifact/test",
                    1,
                    [2, 2],
                    &[255; 16]
                ));
                assert!(!paint_rgba_image(
                    ui,
                    rect,
                    "qnc://local/artifact/invalid",
                    1,
                    [2, 2],
                    &[255; 4]
                ));
            });
        });
        assert!(output
            .textures_delta
            .set
            .iter()
            .any(|(_, delta)| delta.image.size() == [2, 2]));
    }
}
