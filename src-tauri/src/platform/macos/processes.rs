//! Xray on darwin matches the file name, the full path or a `/`-terminated folder.
//! Apps are listed as their bundle folder so helper processes match; Safari never does.
use crate::domain::errors::{BackendError, Result};

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
    let error = || BackendError::new("CORE_START_FAILED", "Could not list running processes");
    // The count races with process creation; leave headroom and retry once.
    let mut pids = Vec::new();
    for _ in 0..2 {
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if count <= 0 {
            return Err(error());
        }
        pids = vec![0 as libc::pid_t; count as usize + 64];
        let size = (pids.len() * std::mem::size_of::<libc::pid_t>()) as libc::c_int;
        let found = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
        if found <= 0 {
            return Err(error());
        }
        let full = found as usize >= pids.len();
        pids.truncate(found as usize);
        if !full {
            break;
        }
    }
    let mut result: Vec<ProcessItem> = pids
        .into_iter()
        .filter(|pid| *pid > 0)
        .filter_map(|pid| {
            let path = super::process::executable(pid)?;
            let path = path.to_str()?;
            // Xray compares paths case-sensitively: keep the kernel's spelling.
            let (id, name) = match bundle(path) {
                Some(bundle) => (
                    format!("{bundle}/"),
                    bundle.rsplit('/').next()?.strip_suffix(".app")?,
                ),
                None => (path.to_owned(), path.rsplit('/').next()?),
            };
            Some(ProcessItem {
                icon: None,
                pid: pid as u32,
                name: name.to_owned(),
                detail: id.trim_end_matches('/').to_owned(),
                id,
            })
        })
        .collect();
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result.dedup_by(|a, b| a.id == b.id);
    Ok(result)
}

/// `/Applications/Discord.app` for any executable inside it, helpers included.
fn bundle(path: &str) -> Option<&str> {
    let end = path.find(".app/")? + ".app".len();
    let bundle = &path[..end];
    (bundle.rsplit('/').next()? != ".app").then_some(bundle)
}

#[cfg(test)]
mod tests {
    #[test]
    fn process_list_contains_this_test_binary() {
        let me = std::env::current_exe().unwrap().canonicalize().unwrap();
        let list = super::processes().unwrap();
        assert!(list.iter().any(|p| std::path::Path::new(&p.detail) == me));
        assert!(list.iter().all(|p| p.id.starts_with(&p.detail)
            && (p.detail.ends_with(&format!("/{}", p.name))
                || p.detail.ends_with(&format!("/{}.app", p.name)))));
        assert!(list.windows(2).all(|w| w[0].id < w[1].id));
    }

    #[test]
    fn helpers_belong_to_the_outermost_bundle() {
        let helper = "/Applications/Discord.app/Contents/Frameworks/Discord Helper.app/Contents/MacOS/Discord Helper";
        assert_eq!(super::bundle(helper), Some("/Applications/Discord.app"));
        assert_eq!(super::bundle("/opt/homebrew/bin/curl"), None);
        assert_eq!(super::bundle("/tmp/.app/x"), None);
    }
}
