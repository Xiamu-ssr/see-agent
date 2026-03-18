use std::fs;
use std::path::Path;

fn main() {
    // Ensure see-agent-corp-web/dist/ exists so rust-embed compiles even without
    // running `trunk build` first. Creates a stub index.html if missing.
    let dist_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../see-agent-corp-web/dist");
    if !dist_dir.exists() {
        fs::create_dir_all(&dist_dir).expect("failed to create see-agent-corp-web/dist");
        fs::write(
            dist_dir.join("index.html"),
            "<!DOCTYPE html><html><body><p>Run <code>trunk build</code> in see-agent-corp-web/ to build the frontend.</p></body></html>",
        )
        .expect("failed to write stub index.html");
    }
}
