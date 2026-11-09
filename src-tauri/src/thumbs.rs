//! What a file looks like in the Library, the way Explorer shows it (after ULTRON's previews.rs).
//!
//! Windows' own shell makes the picture (`IShellItemImageFactory`): a thumbnail where Windows has
//! one (pictures, video frames, PDF and Office pages), else the file's icon. Programs, links, and
//! folders get their icon only. Nothing is opened or run by this side: Windows' thumbnail handlers
//! read the file as they do for Explorer.
//!
//! Pictures are kept as small PNGs in `cache\thumbs`, keyed by the file's id, size, and change
//! time, so a changed file gets a new picture and the old one is removed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use base64::Engine;
use serde::Serialize;

/// Asked for in physical pixels: tiles are 112 logical px at up to 150 % scale.
const PICTURE_PX: i32 = 176;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbKind {
    /// The file's own content: shown filling the tile.
    Thumbnail,
    /// The file's icon: shown on the tile, not stretched.
    Icon,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumb {
    /// `data:image/png;base64,...`
    pub image: String,
    pub kind: ThumbKind,
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

/// The picture for a kept file. Call on a thread that joined COM (shell::on_com_thread).
pub fn thumb(cache: &Path, file_id: &str, path: &Path, icon_only: bool) -> Option<Thumb> {
    let meta = fs::metadata(path).ok()?;
    let changed = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let folder = cache.join("thumbs");
    let stem = format!("{file_id}-{changed}-{}", meta.len());
    for kind in [ThumbKind::Thumbnail, ThumbKind::Icon] {
        let kept = folder.join(format!("{stem}-{}.png", tag(kind)));
        if let Ok(bytes) = fs::read(&kept) {
            return Some(Thumb { image: data_url(&bytes), kind });
        }
    }
    let wants_thumbnail = !icon_only && !meta.is_dir();
    let (png, kind) = wants_thumbnail
        .then(|| shell_png(path, true).map(|png| (png, ThumbKind::Thumbnail)))
        .flatten()
        .or_else(|| shell_png(path, false).map(|png| (png, ThumbKind::Icon)))?;
    if fs::create_dir_all(&folder).is_ok() {
        forget(&folder, file_id);
        let _ = fs::write(folder.join(format!("{stem}-{}.png", tag(kind))), &png);
    }
    Some(Thumb { image: data_url(&png), kind })
}

fn tag(kind: ThumbKind) -> &'static str {
    match kind {
        ThumbKind::Thumbnail => "t",
        ThumbKind::Icon => "i",
    }
}

/// Remove a file's older pictures.
fn forget(folder: &Path, file_id: &str) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    let prefix = format!("{file_id}-");
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Straight-alpha RGBA from the shell's premultiplied BGRA. A bitmap whose alpha is all zero
/// (no alpha channel at all) is opaque.
fn to_rgba(bgra: &[u8]) -> Vec<u8> {
    let (pixels, _) = bgra.as_chunks::<4>();
    let opaque = pixels.iter().all(|&[_, _, _, a]| a == 0);
    let mut out = Vec::with_capacity(bgra.len());
    for &[b, g, r, a] in pixels {
        let (b, g, r, a) = (u32::from(b), u32::from(g), u32::from(r), u32::from(a));
        if opaque {
            out.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
        } else if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let un = |c: u32| ((c * 255 + a / 2) / a).min(255) as u8;
            out.extend_from_slice(&[un(r), un(g), un(b), a as u8]);
        }
    }
    out
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(bytes)
}

#[cfg(windows)]
fn shell_png(path: &Path, thumbnail: bool) -> Option<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY, SIIGBF_THUMBNAILONLY};

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: COM calls on a thread that joined an apartment, with a NUL-terminated path that
    // outlives the call; the bitmap and the DC made here are released before return.
    unsafe {
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let flags = if thumbnail { SIIGBF_THUMBNAILONLY } else { SIIGBF_ICONONLY };
        let bitmap = factory.GetImage(SIZE { cx: PICTURE_PX, cy: PICTURE_PX }, flags | SIIGBF_BIGGERSIZEOK).ok()?;
        let object = HGDIOBJ(bitmap.0);
        let mut info = BITMAP::default();
        let read = GetObjectW(object, size_of::<BITMAP>() as i32, Some((&mut info as *mut BITMAP).cast()));
        let (width, height) = (info.bmWidth, info.bmHeight.abs());
        let mut pixels = vec![0u8; (width.max(0) * height * 4) as usize];
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = CreateCompatibleDC(None);
        let lines = if read > 0 && width > 0 && height > 0 {
            GetDIBits(dc, bitmap, 0, height as u32, Some(pixels.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS)
        } else {
            0
        };
        let _ = DeleteDC(dc);
        let _ = DeleteObject(object);
        if lines != height || width <= 0 {
            return None;
        }
        encode_png(width as u32, height as u32, &to_rgba(&pixels))
    }
}

#[cfg(not(windows))]
fn shell_png(_path: &Path, _thumbnail: bool) -> Option<Vec<u8>> {
    None
}

/// The cache folder's size, for Pengaturan later; also used by tests.
#[allow(dead_code)]
pub fn cached(cache: &Path) -> Vec<PathBuf> {
    fs::read_dir(cache.join("thumbs")).map(|entries| entries.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplied_alpha_is_undone_and_alphaless_bitmaps_are_opaque() {
        assert_eq!(to_rgba(&[10, 20, 30, 0]), [30, 20, 10, 255]);
        assert_eq!(to_rgba(&[0, 0, 128, 128, 0, 0, 0, 0]), [255, 0, 0, 128, 0, 0, 0, 0]);
    }

    #[test]
    fn a_picture_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("catatan.txt");
        fs::write(&file, b"vlan").unwrap();
        let cache = dir.path().join("cache");
        let made = crate::shell::on_com_thread({
            let (cache, file) = (cache.clone(), file.clone());
            move || thumb(&cache, "f1", &file, false)
        })
        .flatten();
        let Some(made) = made else {
            // Some machines (a server core, a CI image) have no shell imaging at all.
            return;
        };
        assert!(made.image.starts_with("data:image/png;base64,"));
        assert_eq!(cached(&cache).len(), 1);
        // Again: read from the cache, no second file.
        let again = thumb(&cache, "f1", &file, false).unwrap();
        assert_eq!(again.image, made.image);
        assert_eq!(cached(&cache).len(), 1);
    }
}
