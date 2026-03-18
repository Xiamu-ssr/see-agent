use axum::body::Body;
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../see-agent-corp-web/dist"]
struct FrontendAssets;

/// Serve embedded frontend assets. Falls back to index.html for SPA routing.
pub async fn serve_frontend(Path(path): Path<String>) -> impl IntoResponse {
    serve_file(&path)
}

/// Serve the root index.html.
pub async fn serve_index() -> impl IntoResponse {
    serve_file("index.html")
}

fn serve_file(path: &str) -> Response {
    // Try the exact path first
    if let Some(content) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // SPA fallback: serve index.html with text/html for client-side routing
    if let Some(content) = FrontendAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
