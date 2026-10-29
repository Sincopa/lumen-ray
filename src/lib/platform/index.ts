// WKWebView on macOS reports a Macintosh user agent; WebView2 never does.
export const isMac =
	typeof navigator !== 'undefined' && navigator.userAgent.includes('Macintosh');

/** Android WebView; the app runs full screen and tunnels through VpnService. */
export const isAndroid =
	typeof navigator !== 'undefined' && /\bAndroid\b/.test(navigator.userAgent);

/** TUN on macOS needs a privileged helper that does not exist yet. */
export const supportsTun = !isMac;

/** Android has no system proxy mode: every connection is the VPN. */
export const supportsSystemProxy = !isAndroid;

/** The Android core is compiled into the APK and updates with it. */
export const supportsCoreUpdate = !isAndroid;

/** Minimize/close buttons, window resizing and the log console window. */
export const isDesktopWindow = !isAndroid;
