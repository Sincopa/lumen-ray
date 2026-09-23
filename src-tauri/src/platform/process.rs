//! Hidden console process, explicit inherited handles, Job Object ownership and graceful stop.
use crate::domain::errors::{BackendError, Result};
use std::{
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::ExitStatusExt,
    },
    path::Path,
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use windows_sys::Win32::{
    Foundation::*,
    Security::SECURITY_ATTRIBUTES,
    System::{Console::*, Pipes::CreatePipe, Threading::*},
};

pub struct CoreProcess {
    handle: OwnedHandle,
    pid: u32,
    _job: super::windows::ProcessJob,
    _logs: Option<crate::diagnostics::CoreLogs>,
}
fn error() -> BackendError {
    BackendError::new("CORE_START_FAILED", "Could not launch managed Xray process")
}
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn pipe() -> Result<(OwnedHandle, OwnedHandle)> {
    unsafe {
        let mut read = std::ptr::null_mut();
        let mut write = std::ptr::null_mut();
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: 1,
        };
        if CreatePipe(&mut read, &mut write, &security, 0) == 0 {
            return Err(error());
        }
        Ok((
            OwnedHandle::from_raw_handle(read),
            OwnedHandle::from_raw_handle(write),
        ))
    }
}
struct Attributes {
    buffer: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn new(handles: &mut [HANDLE]) -> Result<Self> {
        unsafe {
            let mut size = 0;
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
            let mut this = Self {
                buffer: vec![0; size.div_ceil(std::mem::size_of::<usize>())],
                initialized: false,
            };
            if InitializeProcThreadAttributeList(this.ptr(), 1, 0, &mut size) == 0 {
                return Err(error());
            }
            this.initialized = true;
            if UpdateProcThreadAttribute(
                this.ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_mut_ptr() as _,
                std::mem::size_of_val(handles),
                std::ptr::null_mut(),
                std::ptr::null(),
            ) == 0
            {
                return Err(error());
            }
            Ok(this)
        }
    }
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_mut_ptr() as _
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            if self.initialized {
                DeleteProcThreadAttributeList(self.ptr());
            }
        }
    }
}
impl CoreProcess {
    pub async fn spawn(core: &Path, config: &[u8]) -> Result<Self> {
        let (mut process, input, output) = Self::launch(core)?;
        process._logs = Some(crate::diagnostics::CoreLogs::from_reader(
            tokio::fs::File::from_std(output),
        ));
        let mut input = tokio::fs::File::from_std(input);
        tokio::time::timeout(Duration::from_secs(10), input.write_all(config))
            .await
            .map_err(|_| error())?
            .map_err(|_| error())?;
        input.flush().await.map_err(|_| error())?;
        drop(input);
        Ok(process)
    }
    fn launch(core: &Path) -> Result<(Self, std::fs::File, std::fs::File)> {
        if let Some(parent) = core.parent() {
            std::env::set_var("XRAY_LOCATION_ASSET", parent);
        }
        let executable = wide(core.as_os_str());
        let directory = wide(core.parent().ok_or_else(error)?.as_os_str());
        // argv[0] is fixed; lpApplicationName selects the exact verified executable.
        let mut arguments = wide(std::ffi::OsStr::new("xray.exe run -config stdin:"));
        let (input_read, input_write) = pipe()?;
        let (output_read, output_write) = pipe()?;
        unsafe {
            if SetHandleInformation(input_write.as_raw_handle() as _, HANDLE_FLAG_INHERIT, 0) == 0
                || SetHandleInformation(output_read.as_raw_handle() as _, HANDLE_FLAG_INHERIT, 0)
                    == 0
            {
                return Err(error());
            }
            let mut inherited = [
                input_read.as_raw_handle() as HANDLE,
                output_write.as_raw_handle() as HANDLE,
            ];
            let mut attributes = Attributes::new(&mut inherited)?;
            let mut startup: STARTUPINFOEXW = std::mem::zeroed();
            startup.StartupInfo.cb = std::mem::size_of_val(&startup) as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
            startup.StartupInfo.wShowWindow = 0;
            startup.StartupInfo.hStdInput = inherited[0];
            startup.StartupInfo.hStdOutput = inherited[1];
            startup.StartupInfo.hStdError = inherited[1];
            startup.lpAttributeList = attributes.ptr();
            let mut info: PROCESS_INFORMATION = std::mem::zeroed();
            if CreateProcessW(
                executable.as_ptr(),
                arguments.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                CREATE_NEW_CONSOLE
                    | CREATE_NEW_PROCESS_GROUP
                    | CREATE_SUSPENDED
                    | EXTENDED_STARTUPINFO_PRESENT,
                std::ptr::null(),
                directory.as_ptr(),
                &startup.StartupInfo,
                &mut info,
            ) == 0
            {
                return Err(error());
            }
            let handle = OwnedHandle::from_raw_handle(info.hProcess);
            let thread = OwnedHandle::from_raw_handle(info.hThread);
            let job = match super::windows::ProcessJob::attach(info.hProcess) {
                Ok(job) => job,
                Err(error) => {
                    TerminateProcess(info.hProcess, 1);
                    return Err(error);
                }
            };
            let process = Self {
                handle,
                pid: info.dwProcessId,
                _job: job,
                _logs: None,
            };
            if ResumeThread(thread.as_raw_handle() as _) == u32::MAX {
                return Err(error());
            }
            Ok((
                process,
                std::fs::File::from(input_write),
                std::fs::File::from(output_read),
            ))
        }
    }
    pub fn try_wait(&self) -> std::io::Result<Option<std::process::ExitStatus>> {
        unsafe {
            let result = WaitForSingleObject(self.handle.as_raw_handle() as _, 0);
            if result == WAIT_TIMEOUT {
                return Ok(None);
            }
            if result != WAIT_OBJECT_0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut code = 0;
            if GetExitCodeProcess(self.handle.as_raw_handle() as _, &mut code) == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Some(std::process::ExitStatus::from_raw(code)))
        }
    }
    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        loop {
            if let Some(exit) = self.try_wait()? {
                return Ok(exit);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    pub async fn kill(&mut self) -> std::io::Result<()> {
        if self.try_wait()?.is_some() {
            return Ok(());
        }
        // An isolated normal/elevated helper attaches only to this child's hidden console.
        let executable = std::env::current_exe()?;
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            crate::services::vpn_service::command(&executable)
                .args(["--lumen-signal", &self.pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status(),
        )
        .await;
        if matches!(result,Ok(Ok(status)) if status.success()) {
            if let Ok(exit) = tokio::time::timeout(Duration::from_secs(3), self.wait()).await {
                let exit = exit?;
                log::info!("Xray graceful shutdown: {exit}");
                return Ok(());
            }
        }
        log::warn!("Xray shutdown timed out; terminating owned process");
        unsafe {
            if TerminateProcess(self.handle.as_raw_handle() as _, 1) == 0
                && self.try_wait()?.is_none()
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        tokio::time::timeout(Duration::from_secs(3), self.wait())
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "Core termination timed out")
            })??;
        Ok(())
    }
}
pub fn signal(pid: u32) -> bool {
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return false;
        }
        let delivered = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) != 0;
        FreeConsole();
        delivered
    }
}
