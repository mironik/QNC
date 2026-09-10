use eframe::egui::{self, Color32, Rect, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::HashMap;

#[derive(Clone)]
struct StreamTexture {
    key: (String, u64, u64),
    handle: TextureHandle,
}

/// One replaceable texture per surface. Paint only: no playback state, clock or fetching.
pub fn paint_stream_frame(
    ui: &mut Ui,
    rect: Rect,
    surface: egui::Id,
    key: (&str, u64, u64),
    size: [usize; 2],
    rgba: &[u8],
) -> bool {
    if !ui.is_rect_visible(rect)
        || size.contains(&0)
        || size[0].checked_mul(size[1]).and_then(|n| n.checked_mul(4)) != Some(rgba.len())
        || rgba.len() > 32 * 1024 * 1024
    {
        return false;
    }
    let key = (key.0.to_string(), key.1, key.2);
    let diagnostic_key = key.clone();
    let mut texture = ui.data(|data| data.get_temp::<StreamTexture>(surface));
    if texture.as_ref().is_none_or(|texture| texture.key != key) {
        let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba);
        if let Some(texture) = &mut texture {
            texture.handle.set(image, TextureOptions::LINEAR);
            texture.key = key;
        } else {
            texture = Some(StreamTexture {
                key,
                handle: ui
                    .ctx()
                    .load_texture("monitor-frame", image, TextureOptions::LINEAR),
            });
        }
        ui.data_mut(|data| data.insert_temp(surface, texture.clone().unwrap()));
    }
    let aspect = size[0] as f32 / size[1] as f32;
    let width = rect.width().min(rect.height() * aspect);
    let fitted = Rect::from_center_size(rect.center(), Vec2::new(width, width / aspect));
    ui.painter().image(
        texture.unwrap().handle.id(),
        fitted,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    static DIAGNOSTICS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *DIAGNOSTICS.get_or_init(|| std::env::var_os("QNC_PLAYER_DIAGNOSTICS").is_some()) {
        let unix_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        static LAST_REPORT_NS: std::sync::OnceLock<std::sync::atomic::AtomicU64> =
            std::sync::OnceLock::new();
        let last_report = LAST_REPORT_NS.get_or_init(|| std::sync::atomic::AtomicU64::new(0));
        let unix_ns_u64 = unix_ns.min(u128::from(u64::MAX)) as u64;
        let previous = last_report.load(std::sync::atomic::Ordering::Relaxed);
        if unix_ns_u64.saturating_sub(previous) >= 250_000_000
            && last_report
                .compare_exchange(
                    previous,
                    unix_ns_u64,
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
        {
            // CPU paint submission only. This is not a GPU/display presentation acknowledgement.
            eprintln!(
                "AV_V session={} generation={} sequence={} unix_ns={}",
                diagnostic_key.0, diagnostic_key.1, diagnostic_key.2, unix_ns
            );
        }
    }
    true
}

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
    fn stream_replaces_one_texture_without_accumulating_playback_frames() {
        let ctx = egui::Context::default();
        let mut handle = None;
        for sequence in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let id = egui::Id::new("test-stream");
                    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(160.0, 90.0));
                    assert!(paint_stream_frame(
                        ui,
                        rect,
                        id,
                        ("session", 1, sequence),
                        [2, 1],
                        &[255; 8]
                    ));
                    let current =
                        ui.data(|data| data.get_temp::<StreamTexture>(id).unwrap().handle.id());
                    if let Some(previous) = handle {
                        assert_eq!(previous, current);
                    }
                    handle = Some(current);
                    assert!(!paint_stream_frame(
                        ui,
                        rect,
                        id,
                        ("session", 1, sequence),
                        [2, 1],
                        &[255; 7]
                    ));
                });
            });
        }
    }
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
