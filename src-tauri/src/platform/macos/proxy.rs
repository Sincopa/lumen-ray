//! Per-service proxies via networksetup, whose entitlement lets admins write them without
//! an auth prompt. Arguments go to the process directly, never through a shell.
use crate::domain::errors::{BackendError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
const NETWORKSETUP: &str = "/usr/sbin/networksetup";
// As on Windows, private addresses go to Xray, whose routing applies LAN bypass and rules.
// mDNS names cannot resolve remotely.
const BYPASS: [&str; 4] = ["localhost", "127.0.0.1", "::1", "*.local"];
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
struct Proxy {
    enabled: bool,
    server: String,
    port: u16,
    authenticated: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
struct AutoProxy {
    enabled: bool,
    url: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
struct Service {
    web: Proxy,
    secure: Proxy,
    socks: Proxy,
    auto: AutoProxy,
    discovery: bool,
    bypass: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    previous: BTreeMap<String, Service>,
    installed: BTreeMap<String, Service>,
}
pub struct SystemProxy {
    journal: PathBuf,
    port: u16,
}
type Run<'a> = &'a (dyn Fn(&[&str]) -> Option<String> + Sync);
fn error() -> BackendError {
    BackendError::new(
        "PROXY_FAILED",
        "Could not update or restore macOS proxy settings",
    )
}
fn networksetup(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(NETWORKSETUP)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    (output.status.success() && !stdout.contains("** Error")).then_some(stdout)
}
fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
        .map(str::trim)
}
fn parse_proxy(text: &str) -> Option<Proxy> {
    Some(Proxy {
        enabled: field(text, "Enabled")? == "Yes",
        server: field(text, "Server")?.to_owned(),
        port: field(text, "Port")?.parse().ok()?,
        authenticated: field(text, "Authenticated Proxy Enabled").is_some_and(|v| v != "0"),
    })
}
fn parse_auto(text: &str) -> Option<AutoProxy> {
    let url = field(text, "URL")?;
    Some(AutoProxy {
        enabled: field(text, "Enabled")? == "Yes",
        url: if url == "(null)" { "" } else { url }.to_owned(),
    })
}
fn parse_bypass(text: &str) -> Vec<String> {
    if text.starts_with("There aren't any bypass domains") {
        return Vec::new();
    }
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
/// Enabled services only; a leading `*` marks a disabled one.
fn parse_services(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| {
            !line.is_empty() && !line.starts_with('*') && !line.starts_with("An asterisk")
        })
        .map(str::to_owned)
        .collect()
}
fn capture_service(run: Run, name: &str) -> Option<Service> {
    let proxy = |verb: &str| run(&[verb, name]).and_then(|text| parse_proxy(&text));
    Some(Service {
        web: proxy("-getwebproxy")?,
        secure: proxy("-getsecurewebproxy")?,
        socks: proxy("-getsocksfirewallproxy")?,
        auto: parse_auto(&run(&["-getautoproxyurl", name])?)?,
        discovery: field(
            &run(&["-getproxyautodiscovery", name])?,
            "Auto Proxy Discovery",
        )? == "On",
        bypass: parse_bypass(&run(&["-getproxybypassdomains", name])?),
    })
}
/// Read in parallel since each call takes tens of ms. `None` means unreadable proxies.
fn capture(run: Run) -> Result<BTreeMap<String, Option<Service>>> {
    let names = parse_services(&run(&["-listallnetworkservices"]).ok_or_else(error)?);
    let services: Vec<_> = std::thread::scope(|scope| {
        let reads: Vec<_> = names
            .iter()
            .map(|name| scope.spawn(move || capture_service(run, name)))
            .collect();
        reads
            .into_iter()
            .map(|read| read.join().ok().flatten())
            .collect()
    });
    Ok(names.into_iter().zip(services).collect())
}
fn install(previous: &Service, port: u16) -> Service {
    let local = Proxy {
        enabled: true,
        server: "127.0.0.1".into(),
        port,
        authenticated: false,
    };
    // A PAC file, WPAD or SOCKS would take precedence over Lumen for some traffic.
    Service {
        web: local.clone(),
        secure: local,
        socks: Proxy {
            enabled: false,
            ..previous.socks.clone()
        },
        auto: AutoProxy {
            enabled: false,
            ..previous.auto.clone()
        },
        discovery: false,
        bypass: BYPASS.iter().map(|s| s.to_string()).collect(),
    }
}
/// A value the user changed while connected is theirs; only ours is reverted.
fn restored(previous: &Service, installed: &Service, current: &Service) -> Service {
    fn pick<T: PartialEq + Clone>(previous: &T, installed: &T, current: &T) -> T {
        if current == installed {
            previous
        } else {
            current
        }
        .clone()
    }
    Service {
        web: pick(&previous.web, &installed.web, &current.web),
        secure: pick(&previous.secure, &installed.secure, &current.secure),
        socks: pick(&previous.socks, &installed.socks, &current.socks),
        auto: pick(&previous.auto, &installed.auto, &current.auto),
        discovery: pick(
            &previous.discovery,
            &installed.discovery,
            &current.discovery,
        ),
        bypass: pick(&previous.bypass, &installed.bypass, &current.bypass),
    }
}
/// Writes only the settings that differ between `from` and `to`.
fn apply(run: Run, name: &str, from: &Service, to: &Service) -> Result<()> {
    let call = |args: &[&str]| run(args).map(|_| ()).ok_or_else(error);
    let state = |on: bool| if on { "on" } else { "off" };
    for (from, to, set, toggle) in [
        (&from.web, &to.web, "-setwebproxy", "-setwebproxystate"),
        (
            &from.secure,
            &to.secure,
            "-setsecurewebproxy",
            "-setsecurewebproxystate",
        ),
        (
            &from.socks,
            &to.socks,
            "-setsocksfirewallproxy",
            "-setsocksfirewallproxystate",
        ),
    ] {
        if (&from.server, from.port) != (&to.server, to.port) {
            // Setting a server also enables it.
            call(&[set, name, &to.server, &to.port.to_string()])?;
            if !to.enabled {
                call(&[toggle, name, "off"])?;
            }
        } else if from.enabled != to.enabled {
            call(&[toggle, name, state(to.enabled)])?;
        }
    }
    if from.auto.url != to.auto.url && !to.auto.url.is_empty() {
        call(&["-setautoproxyurl", name, &to.auto.url])?;
        if !to.auto.enabled {
            call(&["-setautoproxystate", name, "off"])?;
        }
    } else if from.auto.enabled != to.auto.enabled {
        call(&["-setautoproxystate", name, state(to.auto.enabled)])?;
    }
    if from.discovery != to.discovery {
        call(&["-setproxyautodiscovery", name, state(to.discovery)])?;
    }
    if from.bypass != to.bypass {
        let mut args = vec!["-setproxybypassdomains", name];
        if to.bypass.is_empty() {
            args.push("Empty");
        } else {
            args.extend(to.bypass.iter().map(String::as_str));
        }
        call(&args)?;
    }
    Ok(())
}
impl SystemProxy {
    pub fn enable(directory: &Path, port: u16) -> Result<Self> {
        Self::enable_with(directory, port, &networksetup)
    }
    fn enable_with(directory: &Path, port: u16, run: Run) -> Result<Self> {
        let journal = directory.join("network-state.json");
        if journal.exists() {
            Self::recover_with(directory, run)?;
        }
        // Unreadable services are left alone; `extend` retries them on network change.
        let previous = readable(capture(run)?);
        if previous.is_empty() {
            return Err(error());
        }
        // networksetup cannot restore a proxy password it never returned.
        if previous.values().any(authenticated) {
            return Err(BackendError::new(
                "PROXY_FAILED",
                "An authenticated proxy is configured in macOS network settings; Lumen will not replace it",
            ));
        }
        let installed = previous
            .iter()
            .map(|(name, service)| (name.clone(), install(service, port)))
            .collect();
        let this = Self { journal, port };
        this.write(&Journal {
            previous,
            installed,
        })?;
        let applied = (|| {
            let journal = this.read()?;
            for (name, installed) in &journal.installed {
                apply(run, name, &journal.previous[name], installed)?;
            }
            Ok(())
        })();
        if let Err(e) = applied {
            this.restore_with(run)?;
            return Err(e);
        }
        Ok(this)
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    /// Installs on services added while connected; the journal is written first.
    pub fn extend(&self) -> Result<()> {
        self.extend_with(&networksetup)
    }
    fn extend_with(&self, run: Run) -> Result<()> {
        // Already restored by a network settings reset.
        if !self.journal.exists() {
            return Ok(());
        }
        let mut journal = self.read()?;
        let added: BTreeMap<_, _> = readable(capture(run)?)
            .into_iter()
            .filter(|(name, service)| {
                !journal.installed.contains_key(name) && !authenticated(service)
            })
            .collect();
        if added.is_empty() {
            return Ok(());
        }
        for (name, previous) in &added {
            journal
                .installed
                .insert(name.clone(), install(previous, self.port));
            journal.previous.insert(name.clone(), previous.clone());
        }
        self.write(&journal)?;
        for name in added.keys() {
            apply(run, name, &journal.previous[name], &journal.installed[name])?;
        }
        Ok(())
    }
    /// Write-ahead journal is protected because existing PAC URLs may contain credentials.
    fn write(&self, journal: &Journal) -> Result<()> {
        use crate::storage::secure::{OsSecretStore, SecretStore};
        let data = serde_json::to_vec(journal).map_err(|_| error())?;
        crate::xray::updater::atomic_write(&self.journal, &OsSecretStore.protect(&data)?)
            .map_err(|_| error())
    }
    fn read(&self) -> Result<Journal> {
        use crate::storage::secure::{OsSecretStore, SecretStore};
        serde_json::from_slice(
            &OsSecretStore.unprotect(&std::fs::read(&self.journal).map_err(|_| error())?)?,
        )
        .map_err(|_| error())
    }
    pub fn restore(&self) -> Result<()> {
        self.restore_with(&networksetup)
    }
    fn restore_with(&self, run: Run) -> Result<()> {
        // Already restored by a network settings reset.
        if !self.journal.exists() {
            return Ok(());
        }
        let journal = self.read()?;
        // Services removed meanwhile are skipped; ones added meanwhile never had our values.
        let current = capture(run)?;
        let mut result = Ok(());
        for (name, installed) in &journal.installed {
            let (Some(previous), Some(current)) = (journal.previous.get(name), current.get(name))
            else {
                continue;
            };
            let applied = current.as_ref().ok_or_else(error).and_then(|current| {
                apply(run, name, current, &restored(previous, installed, current))
            });
            if let Err(e) = applied {
                result = Err(e);
            }
        }
        // Keep the journal so the next start retries what failed.
        result?;
        std::fs::remove_file(&self.journal).map_err(|_| error())?;
        Ok(())
    }
    pub fn recover(directory: &Path) -> Result<()> {
        Self::recover_with(directory, &networksetup)
    }
    fn recover_with(directory: &Path, run: Run) -> Result<()> {
        let this = Self {
            journal: directory.join("network-state.json"),
            port: 0,
        };
        if !this.journal.exists() || this.read().is_ok() {
            return this.restore_with(run);
        }
        // Lost Keychain key or damaged file: turn off proxies that exactly match Lumen's
        // so nothing points at a dead listener, and set the journal aside.
        log::warn!("The macOS proxy journal is unreadable; turning off Lumen's proxies");
        let mut result = Ok(());
        for (name, current) in capture(run)? {
            let Some(current) = current.filter(|s| *s == install(s, s.web.port)) else {
                continue;
            };
            let off = Service {
                web: Proxy {
                    enabled: false,
                    ..current.web.clone()
                },
                secure: Proxy {
                    enabled: false,
                    ..current.secure.clone()
                },
                ..current.clone()
            };
            if let Err(e) = apply(run, &name, &current, &off) {
                result = Err(e);
            }
        }
        result?;
        std::fs::rename(&this.journal, directory.join("network-state.unreadable"))
            .map_err(|_| error())
    }
}
fn readable(services: BTreeMap<String, Option<Service>>) -> BTreeMap<String, Service> {
    let all = services.len();
    let readable: BTreeMap<_, _> = services
        .into_iter()
        .filter_map(|(name, service)| Some((name, service?)))
        .collect();
    if readable.len() < all {
        log::warn!(
            "{} macOS network services have unreadable proxies and keep their own",
            all - readable.len()
        );
    }
    readable
}
fn authenticated(service: &Service) -> bool {
    service.web.authenticated || service.secure.authenticated || service.socks.authenticated
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn networksetup_output_parses() {
        let proxy = parse_proxy(
            "Enabled: Yes\nServer: proxy.corp\nPort: 3128\nAuthenticated Proxy Enabled: 0\n",
        )
        .unwrap();
        assert_eq!(
            proxy,
            Proxy {
                enabled: true,
                server: "proxy.corp".into(),
                port: 3128,
                authenticated: false
            }
        );
        let empty = parse_proxy("Enabled: No\nServer: \nPort: 0\nAuthenticated Proxy Enabled: 1\n")
            .unwrap();
        assert!(!empty.enabled && empty.server.is_empty() && empty.authenticated);
        assert_eq!(
            parse_proxy("** Error: Unable to find item in network database.\n"),
            None
        );
        assert_eq!(
            parse_auto("URL: (null)\nEnabled: No\n").unwrap(),
            AutoProxy::default()
        );
        assert_eq!(
            parse_auto("URL: http://wpad/x.pac?a=b: c\nEnabled: Yes\n")
                .unwrap()
                .url,
            "http://wpad/x.pac?a=b: c"
        );
        assert_eq!(
            parse_bypass("*.local\n169.254/16\n"),
            ["*.local", "169.254/16"]
        );
        assert!(parse_bypass("There aren't any bypass domains set on Happ.\n").is_empty());
        assert_eq!(
            parse_services("An asterisk (*) denotes that a network service is disabled.\nWi-Fi\n*Old VPN\nUSB 10/100 LAN\n"),
            ["Wi-Fi", "USB 10/100 LAN"]
        );
    }

    struct Fake(Mutex<BTreeMap<String, Service>>);
    impl Fake {
        fn new(services: &[(&str, Service)]) -> Self {
            Self(Mutex::new(
                services
                    .iter()
                    .map(|(n, s)| (n.to_string(), s.clone()))
                    .collect(),
            ))
        }
        fn get(&self, name: &str) -> Service {
            self.0.lock().unwrap()[name].clone()
        }
        fn set(&self, name: &str, change: impl FnOnce(&mut Service)) {
            change(self.0.lock().unwrap().get_mut(name).unwrap())
        }
        fn run(&self, args: &[&str]) -> Option<String> {
            let mut all = self.0.lock().unwrap();
            if args == ["-listallnetworkservices"] {
                let mut text =
                    "An asterisk (*) denotes that a network service is disabled.\n".to_string();
                for name in all.keys() {
                    text.push_str(name);
                    text.push('\n');
                }
                return Some(text);
            }
            let s = all.get_mut(args[1])?;
            let show = |p: &Proxy| {
                format!(
                    "Enabled: {}\nServer: {}\nPort: {}\nAuthenticated Proxy Enabled: {}\n",
                    if p.enabled { "Yes" } else { "No" },
                    p.server,
                    p.port,
                    p.authenticated as u8
                )
            };
            let on = || args[2] == "on";
            let set = |p: &mut Proxy| {
                p.server = args[2].into();
                p.port = args[3].parse().unwrap();
                p.enabled = true;
            };
            Some(match args[0] {
                "-getwebproxy" => show(&s.web),
                "-getsecurewebproxy" => show(&s.secure),
                "-getsocksfirewallproxy" => show(&s.socks),
                "-getautoproxyurl" => format!(
                    "URL: {}\nEnabled: {}\n",
                    if s.auto.url.is_empty() {
                        "(null)"
                    } else {
                        &s.auto.url
                    },
                    if s.auto.enabled { "Yes" } else { "No" }
                ),
                "-getproxyautodiscovery" => format!(
                    "Auto Proxy Discovery: {}\n",
                    if s.discovery { "On" } else { "Off" }
                ),
                "-getproxybypassdomains" if s.bypass.is_empty() => {
                    format!("There aren't any bypass domains set on {}.\n", args[1])
                }
                "-getproxybypassdomains" => s.bypass.join("\n") + "\n",
                "-setwebproxy" => {
                    set(&mut s.web);
                    String::new()
                }
                "-setsecurewebproxy" => {
                    set(&mut s.secure);
                    String::new()
                }
                "-setsocksfirewallproxy" => {
                    set(&mut s.socks);
                    String::new()
                }
                "-setwebproxystate" => {
                    s.web.enabled = on();
                    String::new()
                }
                "-setsecurewebproxystate" => {
                    s.secure.enabled = on();
                    String::new()
                }
                "-setsocksfirewallproxystate" => {
                    s.socks.enabled = on();
                    String::new()
                }
                "-setautoproxyurl" => {
                    s.auto = AutoProxy {
                        enabled: true,
                        url: args[2].into(),
                    };
                    String::new()
                }
                "-setautoproxystate" => {
                    s.auto.enabled = on();
                    String::new()
                }
                "-setproxyautodiscovery" => {
                    s.discovery = on();
                    String::new()
                }
                "-setproxybypassdomains" if args[2] == "Empty" => {
                    s.bypass.clear();
                    String::new()
                }
                "-setproxybypassdomains" => {
                    s.bypass = args[2..].iter().map(|s| s.to_string()).collect();
                    String::new()
                }
                _ => return None,
            })
        }
    }
    fn store() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        (dir, path)
    }
    fn wifi() -> Service {
        Service {
            web: Proxy {
                enabled: false,
                server: "corp".into(),
                port: 8080,
                authenticated: false,
            },
            socks: Proxy {
                enabled: true,
                server: "socks.lan".into(),
                port: 1080,
                authenticated: false,
            },
            auto: AutoProxy {
                enabled: true,
                url: "http://wpad/proxy.pac".into(),
            },
            discovery: true,
            bypass: vec!["*.local".into(), "169.254/16".into()],
            ..Default::default()
        }
    }

    #[test]
    fn install_points_every_service_at_lumen_and_restore_returns_it() {
        let (_dir, data) = store();
        let fake = Fake::new(&[
            ("Wi-Fi", wifi()),
            ("Thunderbolt Bridge", Service::default()),
        ]);
        let run = |args: &[&str]| fake.run(args);
        let proxy = SystemProxy::enable_with(&data, 10809, &run).unwrap();
        for name in ["Wi-Fi", "Thunderbolt Bridge"] {
            let s = fake.get(name);
            assert!(s.web.enabled && s.secure.enabled);
            assert_eq!((s.web.server.as_str(), s.web.port), ("127.0.0.1", 10809));
            assert!(!s.socks.enabled && !s.auto.enabled && !s.discovery);
            assert!(s.bypass.contains(&"localhost".to_string()));
        }
        proxy.restore_with(&run).unwrap();
        assert_eq!(fake.get("Wi-Fi"), wifi());
        let bridge = fake.get("Thunderbolt Bridge");
        assert!(!bridge.web.enabled && !bridge.secure.enabled);
        assert!(bridge.web.server.is_empty() && bridge.bypass.is_empty());
        assert!(!data.join("network-state.json").exists());
    }

    #[test]
    fn restore_keeps_what_the_user_changed_and_tolerates_service_changes() {
        let (_dir, data) = store();
        let fake = Fake::new(&[("Wi-Fi", wifi()), ("USB LAN", Service::default())]);
        let run = |args: &[&str]| fake.run(args);
        let proxy = SystemProxy::enable_with(&data, 10809, &run).unwrap();
        fake.set("Wi-Fi", |s| {
            s.web.server = "user.proxy".into();
            s.auto.enabled = true;
        });
        fake.0.lock().unwrap().remove("USB LAN");
        fake.0
            .lock()
            .unwrap()
            .insert("iPhone USB".into(), Service::default());
        proxy.restore_with(&run).unwrap();
        let s = fake.get("Wi-Fi");
        assert_eq!((s.web.server.as_str(), s.web.enabled), ("user.proxy", true));
        assert_eq!(s.auto, wifi().auto);
        assert_eq!(
            (s.socks.clone(), s.discovery, s.bypass.clone()),
            (wifi().socks, true, wifi().bypass)
        );
        assert_eq!(s.secure, wifi().secure);
        assert_eq!(fake.get("iPhone USB"), Service::default());
    }

    #[test]
    fn crash_journal_is_recovered_before_the_next_install() {
        let (_dir, data) = store();
        let fake = Fake::new(&[("Wi-Fi", wifi())]);
        let run = |args: &[&str]| fake.run(args);
        let first = SystemProxy::enable_with(&data, 10809, &run).unwrap();
        std::mem::forget(first);
        let second = SystemProxy::enable_with(&data, 20000, &run).unwrap();
        assert_eq!(fake.get("Wi-Fi").web.port, 20000);
        second.restore_with(&run).unwrap();
        assert_eq!(fake.get("Wi-Fi"), wifi());
        SystemProxy::recover_with(&data, &run).unwrap();
    }

    #[test]
    fn authenticated_proxies_are_never_replaced() {
        let (_dir, data) = store();
        let mut corp = wifi();
        corp.secure.authenticated = true;
        let fake = Fake::new(&[("Wi-Fi", corp.clone())]);
        let run = |args: &[&str]| fake.run(args);
        assert!(SystemProxy::enable_with(&data, 10809, &run).is_err());
        assert_eq!(fake.get("Wi-Fi"), corp);
        assert!(!data.join("network-state.json").exists());
    }

    #[test]
    #[ignore]
    fn real_networksetup_round_trip_on_one_service() {
        let service = std::env::var("LUMEN_PROXY_TEST_SERVICE").unwrap();
        let (_dir, data) = store();
        let run = |args: &[&str]| {
            if args == ["-listallnetworkservices"] {
                return Some(format!(
                    "An asterisk (*) denotes that a network service is disabled.\n{service}\n"
                ));
            }
            assert_eq!(args.get(1), Some(&service.as_str()));
            networksetup(args)
        };
        let before = capture_service(&run, &service).unwrap();
        let proxy = SystemProxy::enable_with(&data, 10809, &run).unwrap();
        assert_eq!(
            capture_service(&run, &service).unwrap(),
            install(&before, 10809)
        );
        proxy.restore_with(&run).unwrap();
        assert_eq!(capture_service(&run, &service).unwrap(), before);
    }

    #[test]
    fn services_added_while_connected_are_installed_and_restored() {
        let (_dir, data) = store();
        let fake = Fake::new(&[("Wi-Fi", wifi())]);
        let run = |args: &[&str]| fake.run(args);
        let proxy = SystemProxy::enable_with(&data, 10809, &run).unwrap();
        let mut usb = wifi();
        usb.web.server = "usb.proxy".into();
        fake.0.lock().unwrap().insert("USB LAN".into(), usb.clone());
        proxy.extend_with(&run).unwrap();
        assert_eq!(fake.get("USB LAN"), install(&usb, 10809));
        proxy.restore_with(&run).unwrap();
        assert_eq!(fake.get("USB LAN"), usb);
        assert_eq!(fake.get("Wi-Fi"), wifi());
        proxy.extend_with(&run).unwrap();
        proxy.restore_with(&run).unwrap();
        assert_eq!(fake.get("USB LAN"), usb);
    }

    #[test]
    fn unreadable_journal_turns_off_only_lumens_proxies() {
        let (_dir, data) = store();
        let other = Service {
            web: Proxy {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7890,
                authenticated: false,
            },
            ..Default::default()
        };
        let fake = Fake::new(&[("Wi-Fi", wifi()), ("Other", other.clone())]);
        let run = |args: &[&str]| fake.run(args);
        std::mem::forget(SystemProxy::enable_with(&data, 10809, &run).unwrap());
        fake.set("Other", |s| *s = other.clone());
        std::fs::write(data.join("network-state.json"), b"\x01damaged").unwrap();
        SystemProxy::recover_with(&data, &run).unwrap();
        let s = fake.get("Wi-Fi");
        assert!(!s.web.enabled && !s.secure.enabled);
        assert_eq!(fake.get("Other"), other);
        assert!(!data.join("network-state.json").exists());
        assert!(data.join("network-state.unreadable").exists());
    }

    #[test]
    fn failed_install_is_rolled_back() {
        let (_dir, data) = store();
        let fake = Fake::new(&[("Wi-Fi", wifi())]);
        let run = |args: &[&str]| {
            if args[0] == "-setproxybypassdomains" && args.len() == 2 + BYPASS.len() {
                return None;
            }
            fake.run(args)
        };
        assert!(SystemProxy::enable_with(&data, 10809, &run).is_err());
        assert_eq!(fake.get("Wi-Fi"), wifi());
        assert!(!data.join("network-state.json").exists());
    }
}
