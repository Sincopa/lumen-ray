//! Local executable icons for the process picker and rule items. No executable is loaded as code.
use base64::Engine;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    Graphics::Gdi::*,
    System::{
        Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        Diagnostics::ToolHelp::*,
        Threading::*,
    },
    UI::{Shell::*, WindowsAndMessaging::*},
};
use winreg::enums::*;
use winreg::RegKey;

static ICON_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static SHGFI_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<String, String>> {
    ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_shgfi_mutex() -> &'static Mutex<()> {
    SHGFI_MUTEX.get_or_init(|| Mutex::new(()))
}

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
        }
    }
}

fn encode_rgba_to_base64_png(width: u32, height: u32, pixels: &[u8]) -> Option<String> {
    let mut bytes = Vec::with_capacity(pixels.len() / 2 + 128);
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().ok()?.write_image_data(pixels).ok()?;
    }
    const PREFIX: &str = "data:image/png;base64,";
    let b64_len = bytes.len().div_ceil(3) * 4;
    let mut out = String::with_capacity(PREFIX.len() + b64_len);
    out.push_str(PREFIX);
    base64::engine::general_purpose::STANDARD.encode_string(&bytes, &mut out);
    Some(out)
}

/// Extract base64-encoded PNG icon from an exact filesystem path (exe, lnk, dll, etc.).
pub fn icon(path: &str) -> Option<String> {
    if path.starts_with(r"\\") || path.contains('\0') {
        return None;
    }
    unsafe {
        let mut resources = Resources {
            icon: std::ptr::null_mut(),
            info: std::mem::zeroed(),
            dc: std::ptr::null_mut(),
        };
        let name: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let mut icon_id = 0;
        let extracted = PrivateExtractIconsW(
            name.as_ptr(),
            0,
            64,
            64,
            &mut resources.icon,
            &mut icon_id,
            1,
            0,
        );
        if extracted == 0 || resources.icon.is_null() {
            let _guard = get_shgfi_mutex().lock().unwrap_or_else(|e| e.into_inner());
            let hr = CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            );
            let need_uninit = hr == 0 || hr == 1;
            let mut file: SHFILEINFOW = std::mem::zeroed();
            let ok = SHGetFileInfoW(
                name.as_ptr(),
                0,
                &mut file,
                std::mem::size_of_val(&file) as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            ) != 0
                || SHGetFileInfoW(
                    name.as_ptr(),
                    0x80, // FILE_ATTRIBUTE_NORMAL
                    &mut file,
                    std::mem::size_of_val(&file) as u32,
                    SHGFI_USEFILEATTRIBUTES | SHGFI_ICON | SHGFI_LARGEICON,
                ) != 0;
            if need_uninit {
                CoUninitialize();
            }
            if ok && !file.hIcon.is_null() {
                resources.icon = file.hIcon;
            } else {
                return None;
            }
        }
        if GetIconInfo(resources.icon, &mut resources.info) == 0 {
            return None;
        }
        let hbm = if !resources.info.hbmColor.is_null() {
            resources.info.hbmColor
        } else if !resources.info.hbmMask.is_null() {
            resources.info.hbmMask
        } else {
            return None;
        };
        let mut bitmap: BITMAP = std::mem::zeroed();
        if GetObjectW(
            hbm,
            std::mem::size_of_val(&bitmap) as i32,
            &mut bitmap as *mut _ as _,
        ) == 0
        {
            return None;
        }
        if !(1..=256).contains(&bitmap.bmWidth) || !(1..=512).contains(&bitmap.bmHeight) {
            return None;
        }
        let target_height = if !resources.info.hbmColor.is_null() {
            bitmap.bmHeight
        } else {
            bitmap.bmHeight / 2
        };
        if target_height <= 0 {
            return None;
        }
        let mut description: BITMAPINFO = std::mem::zeroed();
        description.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        description.bmiHeader.biWidth = bitmap.bmWidth;
        description.bmiHeader.biHeight = -target_height;
        description.bmiHeader.biPlanes = 1;
        description.bmiHeader.biBitCount = 32;
        description.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; (bitmap.bmWidth * target_height * 4) as usize];
        resources.dc = GetDC(std::ptr::null_mut());
        if resources.dc.is_null()
            || GetDIBits(
                resources.dc,
                hbm,
                0,
                target_height as u32,
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
                let alpha = u16::from(pixel[3]);
                for channel in &mut pixel[..3] {
                    *channel = (u16::from(*channel) * 255 / alpha).min(255) as u8;
                }
            }
        }
        encode_rgba_to_base64_png(bitmap.bmWidth as u32, target_height as u32, &pixels)
    }
}

/// Locate the full executable path for an application given either its full path or file name.
pub fn resolve_executable_path(name_or_path: &str) -> Option<PathBuf> {
    let clean = name_or_path.trim().trim_matches('"');
    if clean.is_empty() || clean.starts_with(r"\\") || clean.contains('\0') {
        return None;
    }
    let candidate = Path::new(clean);
    if candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    let file_name = candidate
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| clean.to_string());
    let has_exe_ext = file_name
        .get(file_name.len().saturating_sub(4)..)
        .is_some_and(|ext| ext.eq_ignore_ascii_case(".exe"));
    let exe_name = if has_exe_ext {
        file_name.clone()
    } else {
        format!("{file_name}.exe")
    };

    // 1. Check running processes
    if let Some(path) = find_in_running_processes(&file_name, &exe_name) {
        return Some(path);
    }

    // 2. Check Windows Registry App Paths (HKCU, HKLM)
    if let Some(path) = find_in_app_paths(&exe_name) {
        return Some(path);
    }

    // 3. Check PATH directories
    if let Some(path) = find_in_path(&file_name, &exe_name) {
        return Some(path);
    }

    // 4. Check common installation directories
    if let Some(path) = find_in_common_dirs(&file_name, &exe_name) {
        return Some(path);
    }

    // 5. Check Start Menu shortcuts
    if let Some(path) = find_in_start_menu(&file_name) {
        return Some(path);
    }

    None
}

fn find_in_running_processes(file_name: &str, exe_name: &str) -> Option<PathBuf> {
    let target1 = file_name.to_lowercase();
    let target2 = exe_name.to_lowercase();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut path_buf = vec![0u16; 4096];
        let mut more = Process32FirstW(snapshot, &mut entry);
        while more != 0 {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let proc_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if proc_name.eq_ignore_ascii_case(&target1)
                || proc_name.eq_ignore_ascii_case(&target2)
                || proc_name.to_lowercase() == target1
                || proc_name.to_lowercase() == target2
            {
                let process =
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if !process.is_null() {
                    let mut size = path_buf.len() as u32;
                    if QueryFullProcessImageNameW(process, 0, path_buf.as_mut_ptr(), &mut size) != 0
                    {
                        let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
                        CloseHandle(process);
                        CloseHandle(snapshot);
                        let p = PathBuf::from(full_path);
                        if p.is_file() {
                            return Some(p);
                        }
                        return None;
                    }
                    CloseHandle(process);
                }
            }
            more = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
    }
    None
}

fn find_in_app_paths(exe_name: &str) -> Option<PathBuf> {
    let subkey = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{exe_name}");
    // 1. HKCU
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(&subkey) {
        if let Ok(val) = key.get_value::<String, _>("") {
            let clean = val.trim().trim_matches('"');
            let p = PathBuf::from(clean);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // 2. HKLM
    if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&subkey) {
        if let Ok(val) = key.get_value::<String, _>("") {
            let clean = val.trim().trim_matches('"');
            let p = PathBuf::from(clean);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // 3. HKLM WOW6432Node
    let wow_subkey =
        format!(r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\{exe_name}");
    if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&wow_subkey) {
        if let Ok(val) = key.get_value::<String, _>("") {
            let clean = val.trim().trim_matches('"');
            let p = PathBuf::from(clean);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn check_dir_pair(dir: &Path, file_name: &str, exe_name: &str) -> Option<PathBuf> {
    let c1 = dir.join(file_name);
    if c1.is_file() {
        return Some(c1);
    }
    if !file_name.eq_ignore_ascii_case(exe_name) {
        let c2 = dir.join(exe_name);
        if c2.is_file() {
            return Some(c2);
        }
    }
    None
}

fn find_in_path(file_name: &str, exe_name: &str) -> Option<PathBuf> {
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if let Some(found) = check_dir_pair(&dir, file_name, exe_name) {
                return Some(found);
            }
        }
    }
    None
}

fn find_in_common_dirs(file_name: &str, exe_name: &str) -> Option<PathBuf> {
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let home = Path::new(&userprofile);
        for sub in [r".gemini\bin", r".gemini\antigravity\bin"] {
            if let Some(found) = check_dir_pair(&home.join(sub), file_name, exe_name) {
                return Some(found);
            }
        }
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let programs = Path::new(&localappdata).join("Programs");
        if let Some(found) = check_dir_pair(&programs, file_name, exe_name) {
            return Some(found);
        }
        if let Ok(entries) = std::fs::read_dir(&programs) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(found) = check_dir_pair(&path, file_name, exe_name) {
                        return Some(found);
                    }
                }
            }
        }
    }
    for env_key in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(pf) = std::env::var(env_key) {
            if let Ok(entries) = std::fs::read_dir(Path::new(&pf)) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(found) = check_dir_pair(&path, file_name, exe_name) {
                            return Some(found);
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_in_start_menu(file_name: &str) -> Option<PathBuf> {
    let clean_stem = file_name
        .strip_suffix(".exe")
        .unwrap_or(file_name)
        .to_lowercase();
    let mut folders = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        folders.push(Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Ok(progdata) = std::env::var("ProgramData") {
        folders.push(Path::new(&progdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }

    for folder in folders {
        if !folder.is_dir() {
            continue;
        }
        let mut stack = vec![folder];
        while let Some(current) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(current) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if let Some(ext) = path.extension() {
                        if ext.eq_ignore_ascii_case("lnk") {
                            let stem = path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_lowercase();
                            if stem == clean_stem || stem.contains(&clean_stem) {
                                return Some(path);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Retrieve an icon for any application name or path, with full resolution and memory caching.
pub fn get_app_icon(name_or_path: &str) -> Option<String> {
    let clean = name_or_path.trim().trim_matches('"');
    let key = clean.to_lowercase();
    if key.is_empty() {
        return None;
    }

    {
        let cache = get_cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = cache.get(&key) {
            return Some(cached.clone());
        }
    }

    let resolved_path = resolve_executable_path(clean);
    let icon_data = if let Some(ref path) = resolved_path {
        icon(&path.to_string_lossy())
    } else if Path::new(clean).is_file() {
        icon(clean)
    } else {
        None
    };

    if let Some(data) = icon_data {
        let mut cache = get_cache().lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(key, data.clone());
        if let Some(p) = resolved_path {
            if let Some(name) = p.file_name() {
                cache.insert(name.to_string_lossy().to_lowercase(), data.clone());
            }
        }
        Some(data)
    } else {
        None
    }
}

/// Batch-resolve icons for a list of rule names/paths.
pub fn get_rule_icons_map(names: &[String]) -> HashMap<String, String> {
    let mut result = HashMap::with_capacity(names.len());
    for name in names {
        let key = name.trim().to_lowercase();
        if key.is_empty() || result.contains_key(&key) {
            continue;
        }
        if let Some(icon_data) = get_app_icon(name) {
            result.insert(key, icon_data);
        }
    }
    result
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

    #[test]
    fn resolves_executable_path_and_cached_file_name() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("lumen-icon-fixture.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();

        let quoted_path = format!("\"{}\"", executable.display());
        let icon = super::get_app_icon(&quoted_path).expect("fixture icon must be resolved");
        assert!(icon.starts_with("data:image/png;base64,iVBOR"));
        assert_eq!(super::get_app_icon("lumen-icon-fixture.exe"), Some(icon));
    }
}
