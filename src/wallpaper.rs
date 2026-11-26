use std::path::PathBuf;

use anyhow::{Context, Result};
use image::RgbaImage;
use image::imageops::FilterType;

pub fn spawn(
    path: String,
    cover: (u32, u32),
    done: impl FnOnce(Result<RgbaImage>) + Send + 'static,
) {
    std::thread::Builder::new()
        .name("wallpaper".into())
        .spawn(move || done(load(&path, cover)))
        .ok();
}

fn load(path: &str, (w, h): (u32, u32)) -> Result<RgbaImage> {
    let path = expand(path);
    let image = image::ImageReader::open(&path)
        .with_context(|| format!("background-image: {}", path.display()))?
        .with_guessed_format()?
        .decode()
        .with_context(|| format!("background-image: {}", path.display()))?
        .into_rgba8();
    let (iw, ih) = image.dimensions();
    let mut s = 8192.0 / iw.max(ih) as f32;
    if w > 0 && h > 0 {
        s = s.min(cover_scale((iw, ih), (w, h)));
    }
    if s >= 1.0 {
        return Ok(image);
    }
    let (nw, nh) = (
        ((iw as f32 * s).ceil() as u32).max(1),
        ((ih as f32 * s).ceil() as u32).max(1),
    );
    Ok(image::imageops::resize(
        &image,
        nw,
        nh,
        FilterType::CatmullRom,
    ))
}

pub fn cover_scale((iw, ih): (u32, u32), (w, h): (u32, u32)) -> f32 {
    (w as f32 / iw as f32).max(h as f32 / ih as f32)
}

fn expand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
    {
        let mut p = PathBuf::from(home);
        p.push(rest.trim_start_matches(['/', '\\']));
        return p;
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers() {
        assert_eq!(cover_scale((1000, 500), (500, 500)), 1.0);
        assert_eq!(cover_scale((4000, 2000), (1000, 1000)), 0.5);
        assert_eq!(cover_scale((1920, 1080), (3840, 1080)), 2.0);
    }
}
