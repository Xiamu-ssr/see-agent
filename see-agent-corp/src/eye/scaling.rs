use base64::Engine;

use crate::consts::{ASPECT_TOLERANCE, SCALE_TARGETS};
use crate::error::{Result, CorpError};

use super::capture::Screenshot;

/// Find the best target resolution for scaling a screenshot.
///
/// Returns `None` if the image is already small enough (no scaling needed).
///
/// `match_mode`: "aspect_ratio" (default) or "pixel_count"
pub fn find_target_resolution(
    width: u32,
    height: u32,
    match_mode: &str,
) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }

    // Filter: only targets where the source exceeds at least one dimension
    let candidates: Vec<(u32, u32)> = SCALE_TARGETS
        .iter()
        .copied()
        .filter(|&(tw, th)| !(tw >= width && th >= height))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    match match_mode {
        "pixel_count" => {
            let src_pixels = width as u64 * height as u64;
            candidates
                .into_iter()
                .min_by_key(|&(tw, th)| {
                    let target_pixels = tw as u64 * th as u64;
                    src_pixels.abs_diff(target_pixels)
                })
        }
        _ => {
            // aspect_ratio mode (default)
            let aspect = width as f64 / height as f64;
            let mut best: Option<(u32, u32, f64)> = None;

            for (tw, th) in candidates {
                let target_aspect = tw as f64 / th as f64;
                let diff = (aspect - target_aspect).abs() / aspect;
                if diff < ASPECT_TOLERANCE
                    && (best.is_none() || diff < best.unwrap().2)
                {
                    best = Some((tw, th, diff));
                }
            }

            best.map(|(tw, th, _)| (tw, th))
        }
    }
}

/// Scale a screenshot to the target resolution.
///
/// Returns a new Screenshot with `screen_width`/`screen_height` set to
/// the original dimensions (for coordinate reverse-mapping).
pub fn scale_screenshot(
    screenshot: &Screenshot,
    target_w: u32,
    target_h: u32,
) -> Result<Screenshot> {
    if screenshot.width == target_w && screenshot.height == target_h {
        return Ok(screenshot.clone());
    }

    // Load image from retained bytes or decode from base64
    let img = if let Some(ref bytes) = screenshot.image_bytes {
        image::load_from_memory(bytes).map_err(|e| CorpError::Agent {
            message: format!("image decode error: {e}"),
        })?
    } else {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&screenshot.base64)
            .map_err(|e| CorpError::Agent {
                message: format!("base64 decode error: {e}"),
            })?;
        image::load_from_memory(&bytes).map_err(|e| CorpError::Agent {
            message: format!("image decode error: {e}"),
        })?
    };

    // Resize with Lanczos
    let resized = img.resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3);

    // Encode to WebP
    let mut webp_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut webp_bytes);
    resized
        .write_to(&mut cursor, image::ImageFormat::WebP)
        .map_err(|e| CorpError::Agent {
            message: format!("webp encode error: {e}"),
        })?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&webp_bytes);

    Ok(Screenshot {
        base64: b64,
        width: target_w,
        height: target_h,
        scale_factor: screenshot.scale_factor,
        mime_type: "image/webp".to_owned(),
        screen_width: Some(screenshot.screen_width.unwrap_or(screenshot.width)),
        screen_height: Some(screenshot.screen_height.unwrap_or(screenshot.height)),
        image_bytes: Some(webp_bytes),
    })
}

/// Reverse-map coordinates from LLM (model) space back to screen space.
///
/// `(x, y)` = coordinates the LLM returned
/// `model_w, model_h` = the dimensions the LLM saw (scaled screenshot)
/// `screen_w, screen_h` = the original logical screen dimensions
pub fn scale_coordinates(
    x: i32,
    y: i32,
    model_w: u32,
    model_h: u32,
    screen_w: u32,
    screen_h: u32,
) -> (i32, i32) {
    if model_w == 0 || model_h == 0 {
        return (x, y);
    }
    let sx = (x as f64 * screen_w as f64 / model_w as f64).round() as i32;
    let sy = (y as f64 * screen_h as f64 / model_h as f64).round() as i32;
    (sx, sy)
}

/// Scale coordinate fields in tool arguments back to screen space.
///
/// Applies `scale_coordinates` to the relevant fields for click, drag, scroll.
pub fn scale_tool_args(
    tool_name: &str,
    args: &mut serde_json::Value,
    model_w: u32,
    model_h: u32,
    screen_w: u32,
    screen_h: u32,
) {
    match tool_name {
        "click" | "scroll" => {
            if let (Some(x), Some(y)) = (
                args.get("x").and_then(|v| v.as_i64()).map(|v| v as i32),
                args.get("y").and_then(|v| v.as_i64()).map(|v| v as i32),
            ) {
                let (sx, sy) = scale_coordinates(x, y, model_w, model_h, screen_w, screen_h);
                args["x"] = serde_json::json!(sx);
                args["y"] = serde_json::json!(sy);
            }
        }
        "drag" => {
            if let (Some(sx), Some(sy)) = (
                args.get("start_x")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32),
                args.get("start_y")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32),
            ) {
                let (nsx, nsy) = scale_coordinates(sx, sy, model_w, model_h, screen_w, screen_h);
                args["start_x"] = serde_json::json!(nsx);
                args["start_y"] = serde_json::json!(nsy);
            }
            if let (Some(ex), Some(ey)) = (
                args.get("end_x")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32),
                args.get("end_y")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32),
            ) {
                let (nex, ney) = scale_coordinates(ex, ey, model_w, model_h, screen_w, screen_h);
                args["end_x"] = serde_json::json!(nex);
                args["end_y"] = serde_json::json!(ney);
            }
        }
        _ => {} // Other tools: pass through unchanged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn find_target_aspect_ratio() {
        // 1728x1117 (Retina MacBook logical) → aspect 1.547
        // Closest within 5%: (1280, 800) ratio=1.600, diff=3.4%
        let result = find_target_resolution(1728, 1117, "aspect_ratio");
        assert_eq!(result, Some((1280, 800)));
    }

    #[test]
    fn find_target_4_3_display() {
        // 1920x1440 → aspect 1.333 → matches (1024, 768) exactly
        let result = find_target_resolution(1920, 1440, "aspect_ratio");
        assert_eq!(result, Some((1024, 768)));
    }

    #[test]
    fn find_target_pixel_count() {
        let result = find_target_resolution(1728, 1117, "pixel_count");
        assert!(result.is_some());
    }

    #[test]
    fn find_target_already_small() {
        // Image smaller than all targets → no scaling
        let result = find_target_resolution(800, 600, "aspect_ratio");
        assert!(result.is_none());
    }

    #[test]
    fn find_target_zero_dims() {
        assert!(find_target_resolution(0, 0, "aspect_ratio").is_none());
        assert!(find_target_resolution(1920, 0, "aspect_ratio").is_none());
    }

    #[test]
    fn scale_coordinates_basic() {
        // LLM sees 1280x800, screen is 1728x1117
        let (sx, sy) = scale_coordinates(640, 400, 1280, 800, 1728, 1117);
        assert_eq!(sx, 864); // 640 * 1728 / 1280 = 864
        assert_eq!(sy, 559); // 400 * 1117 / 800 = 558.5 → 559
    }

    #[test]
    fn scale_coordinates_identity() {
        let (sx, sy) = scale_coordinates(100, 200, 1280, 800, 1280, 800);
        assert_eq!(sx, 100);
        assert_eq!(sy, 200);
    }

    #[test]
    fn scale_coordinates_zero_model() {
        let (sx, sy) = scale_coordinates(100, 200, 0, 0, 1280, 800);
        assert_eq!(sx, 100);
        assert_eq!(sy, 200);
    }

    #[test]
    fn scale_tool_args_click() {
        let mut args = json!({"x": 640, "y": 400});
        scale_tool_args("click", &mut args, 1280, 800, 1728, 1117);
        assert_eq!(args["x"], 864);
        assert_eq!(args["y"], 559);
    }

    #[test]
    fn scale_tool_args_drag() {
        let mut args = json!({"start_x": 100, "start_y": 100, "end_x": 640, "end_y": 400});
        scale_tool_args("drag", &mut args, 1280, 800, 1728, 1117);
        assert_eq!(args["end_x"], 864);
    }

    #[test]
    fn scale_tool_args_passthrough() {
        let mut args = json!({"command": "ls"});
        let original = args.clone();
        scale_tool_args("shell", &mut args, 1280, 800, 1728, 1117);
        assert_eq!(args, original);
    }

    #[test]
    fn find_target_1920x1080() {
        // 1920x1080 → aspect 1.778
        // Closest: (1366, 768) ratio=1.779, diff < 0.1%
        let result = find_target_resolution(1920, 1080, "aspect_ratio");
        assert_eq!(result, Some((1366, 768)));
    }

    #[test]
    fn find_target_2560x1600() {
        // 2560x1600 → aspect 1.600
        // Exact match: (1280, 800) ratio=1.600
        let result = find_target_resolution(2560, 1600, "aspect_ratio");
        assert_eq!(result, Some((1280, 800)));
    }
}
