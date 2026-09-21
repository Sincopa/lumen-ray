use crate::domain::errors::{BackendError, Result};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::*,
};
pub struct ProcessJob(HANDLE);
unsafe impl Send for ProcessJob {}
impl ProcessJob {
    pub fn attach(process: HANDLE) -> Result<Self> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(error());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(job, process) == 0
            {
                CloseHandle(job);
                return Err(error());
            }
            Ok(Self(job))
        }
    }
}
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn error() -> BackendError {
    BackendError::new(
        "CORE_START_FAILED",
        "Could not establish managed process lifetime",
    )
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessItem {
    pub id: String,
    pub pid: u32,
    pub name: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}
pub fn processes() -> Result<Vec<ProcessItem>> {
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{Diagnostics::ToolHelp::*, Threading::*},
    };
    let mut result = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(error());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut more = Process32FirstW(snapshot, &mut entry);
        while more != 0 {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
            if !process.is_null() {
                let mut path = vec![0u16; 32768];
                let mut size = path.len() as u32;
                if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0 {
                    let detail = String::from_utf16_lossy(&path[..size as usize]);
                    let name = String::from_utf16_lossy(
                        &entry.szExeFile[..entry
                            .szExeFile
                            .iter()
                            .position(|c| *c == 0)
                            .unwrap_or(entry.szExeFile.len())],
                    );
                    result.push(ProcessItem {
                        icon: None,
                        id: detail.to_lowercase(),
                        pid: entry.th32ProcessID,
                        name,
                        detail,
                    });
                }
                CloseHandle(process);
            }
            more = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result.dedup_by(|a, b| a.id == b.id);
    for process in &mut result {
        process.icon = super::icons::icon(&process.detail);
    }
    Ok(result)
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|_| BackendError::new("STORAGE_FAILED", "Failed to access Windows Run registry"))?;
    if enabled {
        let exe = std::env::current_exe()
            .map_err(|_| BackendError::new("STORAGE_FAILED", "Executable path is unavailable"))?;
        let cmd = format!("\"{}\" --minimized", exe.to_string_lossy());
        key.set_value("Lumen", &cmd)
            .map_err(|_| BackendError::new("STORAGE_FAILED", "Failed to register autostart"))?;
    } else {
        let _ = key.delete_value("Lumen");
    }
    Ok(())
}

pub fn is_autostart() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        key.get_value::<String, _>("Lumen").is_ok()
    } else {
        false
    }
}
