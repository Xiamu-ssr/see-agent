use async_trait::async_trait;

use crate::error::Result;

/// A screen screenshot with metadata.
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// Base64-encoded image data (no data-URI prefix).
    pub base64: String,
    /// Logical width in CSS/point pixels.
    pub width: u32,
    /// Logical height in CSS/point pixels.
    pub height: u32,
    /// Physical-to-logical pixel ratio (e.g. 2.0 on Retina).
    pub scale_factor: f64,
    /// MIME type of the encoded image.
    pub mime_type: String,
    /// Original screen width before LLM scaling (None = no scaling applied).
    pub screen_width: Option<u32>,
    /// Original screen height before LLM scaling.
    pub screen_height: Option<u32>,
    /// Raw image bytes (retained to avoid re-decoding base64 for downstream scaling).
    pub image_bytes: Option<Vec<u8>>,
}

impl Screenshot {
    pub fn physical_width(&self) -> u32 {
        (self.width as f64 * self.scale_factor) as u32
    }

    pub fn physical_height(&self) -> u32 {
        (self.height as f64 * self.scale_factor) as u32
    }

    /// OpenAI vision detail level: "low" if both dims <= threshold, else "high".
    pub fn detail(&self) -> &str {
        if self.width <= crate::consts::VISION_LOW_DETAIL_MAX_DIM
            && self.height <= crate::consts::VISION_LOW_DETAIL_MAX_DIM
        {
            "low"
        } else {
            "high"
        }
    }

    /// Save screenshot to disk.
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &self.base64,
        )
        .map_err(|e| crate::error::SeeError::Agent {
            message: format!("base64 decode error: {e}"),
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

/// Screen capture trait — platform implementations provide this.
#[async_trait]
pub trait Eye: Send + Sync {
    /// Capture the current screen and return a Screenshot.
    async fn capture(&self) -> Result<Screenshot>;
}

/// macOS screen capture using the `screencapture` CLI tool.
#[cfg(target_os = "macos")]
pub struct MacEye {
    scale_factor: std::sync::OnceLock<f64>,
}

#[cfg(target_os = "macos")]
impl MacEye {
    pub fn new() -> Self {
        Self {
            scale_factor: std::sync::OnceLock::new(),
        }
    }

    fn detect_scale_factor(&self) -> f64 {
        // Try system_profiler for Retina detection
        if let Ok(output) = std::process::Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("Retina") {
                return 2.0;
            }
        }
        1.0
    }
}

#[cfg(target_os = "macos")]
impl Default for MacEye {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
#[async_trait]
impl Eye for MacEye {
    async fn capture(&self) -> Result<Screenshot> {
        use base64::Engine;

        let scale = *self
            .scale_factor
            .get_or_init(|| self.detect_scale_factor());

        // Use screencapture CLI to capture screen
        let tmp_path = std::env::temp_dir().join("see-agent-screenshot.png");
        let output = tokio::process::Command::new("screencapture")
            .args(["-x", "-t", "png", tmp_path.to_str().unwrap()])
            .output()
            .await
            .map_err(|e| crate::error::SeeError::Agent {
                message: format!("screencapture failed: {e}"),
            })?;

        if !output.status.success() {
            return Err(crate::error::SeeError::Agent {
                message: "screencapture returned non-zero".to_owned(),
            });
        }

        let png_bytes = std::fs::read(&tmp_path)?;
        let _ = std::fs::remove_file(&tmp_path);

        // Decode to get dimensions
        let img = image::load_from_memory(&png_bytes).map_err(|e| {
            crate::error::SeeError::Agent {
                message: format!("image decode error: {e}"),
            }
        })?;

        let physical_w = img.width();
        let physical_h = img.height();

        // Downscale to logical resolution if Retina
        let (logical_img, logical_w, logical_h) = if scale > 1.0 {
            let lw = (physical_w as f64 / scale) as u32;
            let lh = (physical_h as f64 / scale) as u32;
            let resized = img.resize_exact(lw, lh, image::imageops::FilterType::Lanczos3);
            (resized, lw, lh)
        } else {
            (img, physical_w, physical_h)
        };

        // Encode to WebP (lossless)
        let mut webp_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut webp_bytes);
        logical_img
            .write_to(&mut cursor, image::ImageFormat::WebP)
            .map_err(|e| crate::error::SeeError::Agent {
                message: format!("webp encode error: {e}"),
            })?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(&webp_bytes);

        Ok(Screenshot {
            base64: b64,
            width: logical_w,
            height: logical_h,
            scale_factor: scale,
            mime_type: "image/webp".to_owned(),
            screen_width: None,
            screen_height: None,
            image_bytes: Some(webp_bytes),
        })
    }
}

/// Linux placeholder — returns an error until implemented.
#[cfg(target_os = "linux")]
pub struct LinuxEye;

#[cfg(target_os = "linux")]
#[async_trait]
impl Eye for LinuxEye {
    async fn capture(&self) -> Result<Screenshot> {
        Err(crate::error::SeeError::Agent {
            message: "Linux screen capture not yet implemented".to_owned(),
        })
    }
}
