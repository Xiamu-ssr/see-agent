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
    // Try the exact path first, then fall back to index.html for SPA routing
    let file = FrontendAssets::get(path)
        .or_else(|| FrontendAssets::get("index.html"));

    match file {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}
