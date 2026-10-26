//! Embedded static dashboard (same origin as tailnet API).

use include_dir::{include_dir, Dir};

static WEB_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../web/static");

/// `GET` paths for the dashboard (`/`, `/index.html`, `/app.js`, `/style.css`).
pub fn lookup_web_asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    let path = path.split('?').next().unwrap_or(path);
    let file_name = match path {
        "/" | "/index.html" => "index.html",
        "/app.js" => "app.js",
        "/style.css" => "style.css",
        _ => return None,
    };
    let file = WEB_DIR.get_file(file_name)?;
    let mime = match file_name {
        "index.html" => "text/html; charset=utf-8",
        "app.js" => "application/javascript; charset=utf-8",
        "style.css" => "text/css; charset=utf-8",
        _ => "application/octet-stream",
    };
    Some((file.contents(), mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_dashboard_assets() {
        assert!(lookup_web_asset("/").is_some());
        assert!(lookup_web_asset("/app.js").is_some());
        assert!(lookup_web_asset("/style.css").is_some());
        assert!(lookup_web_asset("/v1/health").is_none());
    }
}
