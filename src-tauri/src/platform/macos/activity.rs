//! App Nap throttles a hidden app: its timers, CPU and network I/O, and the
//! processes it started. Minutes after the window was closed to the menu bar,
//! the Xray core that carries the system proxy and Lumen's own connection
//! checks slowed down until the checks gave up and the session was released.
//! A session that is up or on its way up holds an activity assertion, which
//! keeps App Nap away and still lets the Mac sleep when idle.
use objc2::{
    rc::{autoreleasepool, Retained},
    runtime::{NSObjectProtocol, ProtocolObject},
};
use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
use std::sync::Mutex;

struct Activity(Retained<ProtocolObject<dyn NSObjectProtocol>>);
// The token is an opaque object; `endActivity:` accepts it from any thread.
unsafe impl Send for Activity {}

static ACTIVITY: Mutex<Option<Activity>> = Mutex::new(None);

/// Holds the assertion while `active`; repeated calls with the same value do nothing.
pub fn keep_active(active: bool) {
    let Ok(mut held) = ACTIVITY.lock() else {
        return;
    };
    if active == held.is_some() {
        return;
    }
    autoreleasepool(|_| {
        let info = NSProcessInfo::processInfo();
        if let Some(Activity(token)) = held.take() {
            // SAFETY: the token came from `beginActivityWithOptions:reason:`.
            unsafe { info.endActivity(&token) };
            log::info!("App Nap allowed again: no connection is active");
        } else {
            let reason = NSString::from_str("Lumen keeps the VPN connection running");
            let token = info.beginActivityWithOptions_reason(
                NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
                &reason,
            );
            *held = Some(Activity(token));
            log::info!("App Nap held off while the connection is active");
        }
    });
}
