//! Local executable icons for the existing process picker. No executable is loaded as code.
use base64::Engine;
use windows_sys::Win32::{
    Graphics::Gdi::*,
    System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
    UI::{Shell::*, WindowsAndMessaging::*},
};

struct Resources {
    icon: HICON,
    info: ICONINFO,
    dc: HDC,
}
impl Drop for Resources {
    fn drop(&mut self) {
        unsafe {
            if !self.info.hbmColor.is_null() {
                DeleteObject(self.info.hbmColor);
            }
            if !self.info.hbmMask.is_null() {
                DeleteObject(self.info.hbmMask);
            }
            if !self.icon.is_null() {
                DestroyIcon(self.icon);
            }
            if !self.dc.is_null() {
                ReleaseDC(std::ptr::null_mut(), self.dc);
            }
            CoUninitialize();
        }
    }
}
pub fn icon(path: &str) -> Option<String> {
    if path.starts_with("\\\\") || path.contains('\0') {
        return None;
    }
    unsafe {
        if CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) < 0 {
            return None;
        }
        let mut resources = Resources {
            icon: std::ptr::null_mut(),
            info: std::mem::zeroed(),
            dc: std::ptr::null_mut(),
        };
        let name: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let mut icon_id = 0;
        if PrivateExtractIconsW(
            name.as_ptr(),
            0,
            64,
            64,
            &mut resources.icon,
            &mut icon_id,
            1,
            0,
        ) == 0
            || resources.icon.is_null()
        {
            let mut file: SHFILEINFOW = std::mem::zeroed();
            if SHGetFileInfoW(
                name.as_ptr(),
                0,
                &mut file,
                std::mem::size_of_val(&file) as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            ) == 0
            {
                return None;
            }
            resources.icon = file.hIcon;
        }
        if GetIconInfo(resources.icon, &mut resources.info) == 0
            || resources.info.hbmColor.is_null()
        {
            return None;
        }
        let mut bitmap: BITMAP = std::mem::zeroed();
        if GetObjectW(
            resources.info.hbmColor,
            std::mem::size_of_val(&bitmap) as i32,
            &mut bitmap as *mut _ as _,
        ) == 0
        {
            return None;
        }
        if !(1..=128).contains(&bitmap.bmWidth) || !(1..=128).contains(&bitmap.bmHeight) {
            return None;
        }
        let mut description: BITMAPINFO = std::mem::zeroed();
        description.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        description.bmiHeader.biWidth = bitmap.bmWidth;
        description.bmiHeader.biHeight = -bitmap.bmHeight;
        description.bmiHeader.biPlanes = 1;
        description.bmiHeader.biBitCount = 32;
        description.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; (bitmap.bmWidth * bitmap.bmHeight * 4) as usize];
        resources.dc = GetDC(std::ptr::null_mut());
        if resources.dc.is_null()
            || GetDIBits(
                resources.dc,
                resources.info.hbmColor,
                0,
                bitmap.bmHeight as u32,
                pixels.as_mut_ptr() as _,
                &mut description,
                DIB_RGB_COLORS,
            ) == 0
        {
            return None;
        }
        let has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            if !has_alpha {
                pixel[3] = 255;
            } else if pixel[3] > 0 {
                for channel in 0..3 {
                    pixel[channel] =
                        (u16::from(pixel[channel]) * 255 / u16::from(pixel[3])).min(255) as u8;
                }
            }
        }
        let mut bytes = Vec::new();
        {
            let mut encoder =
                png::Encoder::new(&mut bytes, bitmap.bmWidth as u32, bitmap.bmHeight as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .ok()?
                .write_image_data(&pixels)
                .ok()?;
        }
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
}
#[cfg(test)]
mod tests {
    use base64::Engine;

    #[test]
    fn extracts_current_executable_icon() {
        let executable = std::env::current_exe().unwrap();
        let icon = super::icon(&executable.to_string_lossy()).unwrap();
        assert!(icon.starts_with("data:image/png;base64,iVBOR"));
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(icon.trim_start_matches("data:image/png;base64,"))
            .unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = decoder.read_info().unwrap();
        assert!(reader.info().width >= 32);
        assert!(reader.info().height >= 32);
    }
}
