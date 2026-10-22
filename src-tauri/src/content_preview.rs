use digview_core::{
    content_preview::{PreviewBytes, PreviewStore},
    engine::Engine,
};
use std::sync::Arc;
use tauri::{
    http::{Request, Response},
    Manager, Runtime, UriSchemeContext, UriSchemeResponder,
};

pub fn serve<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let origin = request
        .headers()
        .get("origin")
        .map(|v| v.to_str().unwrap_or("invalid"));
    let allowed = origin.is_none_or(|origin| {
        matches!(
            origin,
            "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
        ) || (cfg!(debug_assertions)
            && matches!(origin, "http://127.0.0.1:1420" | "http://localhost:1420"))
    });
    if context.webview_label() != "main" || !allowed {
        responder.respond(
            Response::builder()
                .status(403)
                .body(Vec::<u8>::new())
                .unwrap(),
        );
        return;
    }
    let origin = origin.map(str::to_owned);
    let app = context.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = (|| {
            let mut builder = Response::builder()
                .header("Cache-Control", "no-store")
                .header("X-Content-Type-Options", "nosniff")
                .header("Content-Security-Policy", "default-src 'none'; sandbox")
                .header("Accept-Ranges", "bytes");
            if let Some(origin) = origin {
                builder = builder
                    .header("Access-Control-Allow-Origin", origin)
                    .header("Vary", "Origin")
                    .header(
                        "Access-Control-Expose-Headers",
                        "Content-Length, Content-Range, Accept-Ranges",
                    );
            }
            let token = request.uri().path().strip_prefix('/').unwrap_or("");
            if token.len() != 64
                || !token
                    .bytes()
                    .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
                || request.uri().query().is_some()
            {
                return builder.status(404).body(vec![]);
            }
            if request.method() == "OPTIONS" {
                return builder
                    .status(204)
                    .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Range")
                    .body(vec![]);
            }
            if request.method() != "GET" && request.method() != "HEAD" {
                return builder.status(405).body(vec![]);
            }
            let Some(store) = app.try_state::<Arc<PreviewStore>>() else {
                return builder.status(503).body(vec![]);
            };
            let Some(engine) = app.try_state::<Engine>() else {
                return builder.status(503).body(vec![]);
            };
            let range = request
                .headers()
                .get("range")
                .map(|value| value.to_str().unwrap_or("invalid"));
            match store.read(&engine, token, range, request.method() == "HEAD") {
                Ok(PreviewBytes {
                    status,
                    mime,
                    total,
                    start,
                    length,
                    bytes,
                }) => {
                    builder = builder
                        .status(status)
                        .header("Content-Type", mime)
                        .header("Content-Length", length);
                    if status == 206 {
                        builder = builder.header(
                            "Content-Range",
                            format!("bytes {start}-{}/{total}", start + length - 1),
                        );
                    }
                    if status == 416 {
                        builder = builder.header("Content-Range", format!("bytes */{total}"));
                    }
                    builder.body(bytes)
                }
                Err(error) => {
                    crate::diagnostics::background_failure(&error);
                    builder
                        .status(404)
                        .header("Content-Type", "text/plain")
                        .body(
                            b"Preview unavailable. Reopen the file or refresh the library."
                                .to_vec(),
                        )
                }
            }
        })();
        responder.respond(response.unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Vec::<u8>::new())
                .unwrap()
        }));
    });
}
