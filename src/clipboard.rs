#[cfg(windows)]
pub fn read_text() -> Option<String> {
    use std::time::Duration;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

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
    let text = unsafe {
        GetClipboardData(CF_UNICODETEXT.0 as u32)
            .ok()
            .and_then(|handle| {
                let global = HGLOBAL(handle.0);
                let ptr = GlobalLock(global) as *const u16;
                if ptr.is_null() {
                    return None;
                }
                let mut len = 0;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                let _ = GlobalUnlock(global);
                Some(text)
            })
    };
    let _ = unsafe { CloseClipboard() };
    text
}

#[cfg(not(windows))]
pub fn read_text() -> Option<String> {
    None
}
