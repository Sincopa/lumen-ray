//! `lumen://` links, such as the "Add subscription" button on a subscription
//! page. A link only fills the add-profile form; importing still needs the
//! user to confirm it there.
use tauri::Emitter;

static IMPORT_LINK: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Far above any real subscription link; a longer one is not read at all.
const MAX_LINK: usize = 65536;
/// What other clients call the import action, so a subscription page set up
/// for them works after swapping their scheme for `lumen`.
const ACTIONS: &[&str] = &[
    "import",
    "add",
    "install-config",
    "install-sub",
    "subscribe",
    "sub",
];

/// Pulls the subscription (or single server) link out of a `lumen://` link.
///
/// Subscription pages such as Remnawave's build the button by appending the
/// subscription URL to a per-app prefix, so every common prefix works:
/// `lumen://import?url=`, `lumen://install-config?name=…&url=` (v2rayNG),
/// `lumen://import/` (Hiddify), `lumen://add/` (Happ), or the link right after
/// `lumen://`. The link itself may be raw, percent-encoded or base64.
pub fn import_source(input: &str) -> Option<String> {
    let input = input.trim();
    if input.len() > MAX_LINK {
        return None;
    }
    let rest = strip_lumen(input)?;
    if has_scheme(rest) {
        return decode(rest);
    }
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (action, tail) = rest.split_at(end);
    if !ACTIONS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(action))
    {
        return None;
    }
    let tail = tail.strip_prefix('/').unwrap_or(tail);
    let source = match tail.strip_prefix('?') {
        Some(query) => query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            key.eq_ignore_ascii_case("url").then_some(value)
        })?,
        None => tail,
    };
    decode(source)
}

fn strip_lumen(input: &str) -> Option<&str> {
    const PREFIX: &str = "lumen://";
    let head = input.get(..PREFIX.len())?;
    head.eq_ignore_ascii_case(PREFIX)
        .then(|| &input[PREFIX.len()..])
}

/// `scheme://` at the start, as in `https://` or `vless://`.
fn has_scheme(text: &str) -> bool {
    let Some((scheme, _)) = text.split_once("://") else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn decode(raw: &str) -> Option<String> {
    let raw = raw.trim();
    // A raw link keeps its own escapes, such as a percent-encoded server name.
    let mut source = if has_scheme(raw) {
        raw.to_owned()
    } else {
        percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .ok()?
            .trim()
            .to_owned()
    };
    if !has_scheme(&source) {
        source = from_base64(&source)?;
    }
    (has_scheme(&source) && !source.chars().any(char::is_control)).then_some(source)
}

fn from_base64(text: &str) -> Option<String> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    use base64::Engine;
    [STANDARD, URL_SAFE, STANDARD_NO_PAD, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|engine| String::from_utf8(engine.decode(text).ok()?).ok())
        .map(|decoded| decoded.trim().to_owned())
}

/// Hands the link from a `lumen://` link to the add-profile form. Returns
/// whether there was one, so the caller can bring the window up.
pub fn queue_import_link(app: &tauri::AppHandle, input: &str) -> bool {
    // Every command line argument of a second launch comes through here.
    if strip_lumen(input.trim()).is_none() {
        return false;
    }
    // The link carries the subscription token, so it is never logged.
    let Some(source) = import_source(input) else {
        log::warn!("Ignored a lumen:// link in an unknown format");
        return false;
    };
    log::info!("Received a subscription link from a lumen:// link");
    if let Ok(mut pending) = IMPORT_LINK.lock() {
        *pending = Some(source);
    }
    let _ = app.emit("import://pending", ());
    true
}

#[tauri::command]
pub fn take_import_link() -> Option<String> {
    IMPORT_LINK.lock().ok()?.take()
}

#[cfg(test)]
mod tests {
    use super::import_source;

    const SUB: &str = "https://sub.example.com/AbC123xyz";

    #[test]
    fn subscription_page_prefixes() {
        for prefix in [
            "lumen://import?url=",
            "lumen://import/?url=",
            "lumen://install-config?url=",
            "lumen://install-config?name=Remnawave&url=",
            "lumen://add/",
            "lumen://import/",
            "lumen://sub/",
            "LUMEN://Import?URL=",
            "lumen://",
        ] {
            let link = format!("{prefix}{SUB}");
            assert_eq!(import_source(&link).as_deref(), Some(SUB), "{link}");
        }
    }

    #[test]
    fn links_as_the_deep_link_plugin_hands_them_over() {
        // Cold starts and Android pass the link through url::Url first.
        for prefix in [
            "lumen://import?url=",
            "lumen://add/",
            "lumen://import/",
            "lumen://install-config?name=Remnawave&url=",
        ] {
            let link = url::Url::parse(&format!("{prefix}{SUB}")).unwrap();
            assert_eq!(import_source(link.as_str()).as_deref(), Some(SUB), "{link}");
        }
    }

    #[test]
    fn encoded_links() {
        let encoded = "https%3A%2F%2Fsub.example.com%2FAbC123xyz";
        assert_eq!(
            import_source(&format!("lumen://import?url={encoded}")).as_deref(),
            Some(SUB)
        );
        assert_eq!(
            import_source(&format!("lumen://add/{encoded}")).as_deref(),
            Some(SUB)
        );
        use base64::Engine;
        let packed = base64::engine::general_purpose::STANDARD.encode(SUB);
        assert_eq!(
            import_source(&format!("lumen://add/{packed}")).as_deref(),
            Some(SUB)
        );
        // A raw link keeps its own escapes and query.
        let raw = "https://sub.example.com/s/abc?name=%D0%9B%D1%8E%D0%BC%D0%B5%D0%BD";
        assert_eq!(
            import_source(&format!("lumen://add/{raw}")).as_deref(),
            Some(raw)
        );
        let server = "vless://00000000-0000-4000-8000-000000000001@example.com:443?type=ws#Example";
        assert_eq!(
            import_source(&format!("lumen://import/{server}")).as_deref(),
            Some(server)
        );
    }

    #[test]
    fn rejects_other_input() {
        for input in [
            "",
            "C:\\Program Files\\Lumen\\app.exe",
            "--minimized",
            "happ://add/https://sub.example.com/x",
            "lumen://",
            "lumen://import",
            "lumen://import?url=",
            "lumen://import?name=x",
            "lumen://settings/https://sub.example.com/x",
            "lumen://add/not a link",
            "lumen://add/https://sub.example.com/a\nb",
        ] {
            assert_eq!(import_source(input), None, "{input:?}");
        }
        let long = format!("lumen://add/https://sub.example.com/{}", "a".repeat(70000));
        assert_eq!(import_source(&long), None);
    }
}
