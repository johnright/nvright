#[cfg_attr(not(windows), allow(dead_code))]
pub enum Clip {
    Text(String),
    Image(Vec<u8>),
}

pub const SAVE_IMAGE: &str = r#"
local png = ...
local file = vim.api.nvim_buf_get_name(0)
local dir = file ~= '' and vim.fs.dirname(file) or vim.fn.getcwd()
local name = os.date('pasted-%Y%m%d-%H%M%S')
local n = 1
while vim.uv.fs_stat(dir .. '/' .. name .. '.png') do
  n = n + 1
  name = os.date('pasted-%Y%m%d-%H%M%S-') .. n
end
name = name .. '.png'
local f = assert(io.open(dir .. '/' .. name, 'wb'))
f:write(png)
f:close()
local text = vim.bo.filetype == 'markdown' and ('![](' .. name .. ')') or name
vim.api.nvim_put({ text }, 'c', vim.fn.mode() ~= 'i', true)
vim.notify('saved ' .. name)
"#;

#[cfg(windows)]
pub fn read() -> Option<Clip> {
    use std::time::Duration;
    use windows::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};

    let opened = (0..10).any(|attempt| {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        unsafe { OpenClipboard(None) }.is_ok()
    });
    if !opened {
        log::warn!("clipboard unavailable: held by another program");
        return None;
    }
    let clip = unsafe { read_text().map(Clip::Text).or_else(|| read_image().map(Clip::Image)) };
    let _ = unsafe { CloseClipboard() };
    clip
}

#[cfg(windows)]
unsafe fn data(format: u32) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::GetClipboardData;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    unsafe {
        let global = HGLOBAL(GetClipboardData(format).ok()?.0);
        let ptr = GlobalLock(global) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, GlobalSize(global)).to_vec();
        let _ = GlobalUnlock(global);
        Some(bytes)
    }
}

#[cfg(windows)]
unsafe fn read_text() -> Option<String> {
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    let bytes = unsafe { data(CF_UNICODETEXT.0 as u32)? };
    let wide: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .take_while(|&c| c != 0)
        .collect();
    Some(String::from_utf16_lossy(&wide)).filter(|t| !t.is_empty())
}

#[cfg(windows)]
unsafe fn read_image() -> Option<Vec<u8>> {
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::Win32::System::Ole::CF_DIB;
    use windows::core::w;

    let png = unsafe { RegisterClipboardFormatW(w!("PNG")) };
    if let Some(bytes) = unsafe { data(png) } {
        return Some(bytes);
    }
    let dib = unsafe { data(CF_DIB.0 as u32)? };
    match dib_to_png(&dib) {
        Ok(png) => Some(png),
        Err(err) => {
            log::warn!("clipboard image: {err}");
            None
        }
    }
}

#[cfg(not(windows))]
pub fn read() -> Option<Clip> {
    None
}

#[cfg_attr(not(windows), allow(dead_code))]
fn dib_to_png(dib: &[u8]) -> Result<Vec<u8>, String> {
    let u32_at = |i: usize| dib.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let u16_at = |i: usize| dib.get(i..i + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap()));
    let (Some(header), Some(bits), Some(compression), Some(colors)) =
        (u32_at(0), u16_at(14), u32_at(16), u32_at(32))
    else {
        return Err("DIB too short".into());
    };
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let palette = match (bits, colors) {
        (b, 0) if b <= 8 => 4 << b,
        (_, n) => n * 4,
    };
    let offset = 14 + header + masks + palette;
    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(14 + dib.len() as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&offset.to_le_bytes());
    bmp.extend_from_slice(dib);

    let mut image = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    if image.pixels().all(|p| p[3] == 0) {
        image.pixels_mut().for_each(|p| p[3] = 255);
    }
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_round_trip() {
        let mut dib = Vec::new();
        for v in [40u32, 2, 1] {
            dib.extend_from_slice(&v.to_le_bytes());
        }
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[0; 24]);
        dib.extend_from_slice(&[0, 0, 255, 255, 0, 0, 0, 0]);
        let png = dib_to_png(&dib).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 255, 255]);
    }
}
