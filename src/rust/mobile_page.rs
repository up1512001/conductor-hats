//! The static client the browser downloads, pinned to the build serving it.

const PAGE: &str = include_str!("../mobile/index.html");
const STYLE: &str = include_str!("../mobile/styles.css");
const SCRIPT: &str = include_str!(concat!(env!("OUT_DIR"), "/mobile.js"));

/// A content fingerprint, so a rebuilt client is never served from a cache.
///
/// Every response here is already `no-store`, which is a privacy rule rather
/// than a freshness one, and a browser that has already parsed a script at an
/// address is entitled to keep it. FNV-1a because this is a cache key, not a
/// signature.
fn fingerprint(body: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in body.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The address the stylesheet is published at for this build.
///
/// In the path rather than a query string. A query is advisory: a proxy is
/// entitled to strip it from its cache key, and several do, which leaves
/// `/mobile.css?v=new` answered from the entry stored for `/mobile.css`. A
/// distinct path cannot collide with the previous build's, because no cache has
/// ever seen it.
pub fn css_path() -> &'static str {
    static PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PATH.get_or_init(|| format!("/mobile.{}.css", fingerprint(STYLE)))
}

pub fn js_path() -> &'static str {
    static PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PATH.get_or_init(|| format!("/mobile.{}.js", fingerprint(SCRIPT)))
}

/// The page with its two asset references pinned to the build being served.
pub fn page() -> &'static str {
    static PINNED: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PINNED.get_or_init(|| {
        PAGE.replace("/mobile.css", css_path())
            .replace("/mobile.js", js_path())
    })
}

pub fn style() -> &'static str {
    STYLE
}

pub fn script() -> &'static str {
    SCRIPT
}

/// Conductor's own mark, read once from the installed application.
///
/// Empty when the copy cannot be read, and the page falls back to its own
/// drawn mark, so a missing or moved application costs the badge and nothing
/// else.
pub fn logo() -> &'static [u8] {
    static MARK: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    MARK.get_or_init(|| {
        crate::patch::asset_bytes(std::path::Path::new(crate::DEV_APP), "conductor-org-icon")
            .unwrap_or_default()
    })
}
