// WKWebView on macOS reports a Macintosh user agent; WebView2 never does.
export const isMac =
	typeof navigator !== 'undefined' && navigator.userAgent.includes('Macintosh');

/** TUN on macOS needs a privileged helper that does not exist yet. */
export const supportsTun = !isMac;
