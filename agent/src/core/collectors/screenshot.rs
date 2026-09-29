use crate::core::metrics::ScreenshotItem;
use crate::notifications::NotificationDispatcher;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;

pub struct ScreenshotCollector;

impl ScreenshotCollector {
    /// Captures a visual snapshot of the primary display (DP-05, RF-AGENT-02-5).
    /// Scales to <= 1920x1080, compresses to JPEG <= 2MB, and triggers native user notification.
    /// Returns Ok(None) if no displays are available (e.g. headless or permission denied).
    pub fn capture_primary(
    ) -> Result<Option<ScreenshotItem>, Box<dyn std::error::Error + Send + Sync>> {
        let monitors = match xcap::Monitor::all() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to query monitors for screenshot: {:?}", e);
                return Ok(None);
            }
        };

        if monitors.is_empty() {
            tracing::debug!("No monitors detected for screenshot capture");
            return Ok(None);
        }

        // Locate primary monitor, or take the first monitor if no monitor is explicitly primary
        let monitor = monitors
            .iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| monitors.first())
            .cloned();

        let Some(monitor) = monitor else {
            return Ok(None);
        };

        let rgba_image = match monitor.capture_image() {
            Ok(img) => img,
            Err(e) => {
                tracing::warn!(
                    "Screen capture failed (may require Screen Recording permission): {:?}",
                    e
                );
                return Ok(None);
            }
        };

        let width = rgba_image.width();
        let height = rgba_image.height();

        // RF-AGENT-02-5: Max resolution scaled to 1920x1080
        let (target_w, target_h, dynamic_img) = if width > 1920 || height > 1080 {
            let ratio = (1920.0 / width as f64).min(1080.0 / height as f64);
            let target_w = ((width as f64 * ratio).round() as u32).max(1);
            let target_h = ((height as f64 * ratio).round() as u32).max(1);
            let resized = image::imageops::resize(
                &rgba_image,
                target_w,
                target_h,
                image::imageops::FilterType::Triangle,
            );
            (target_w, target_h, image::DynamicImage::ImageRgba8(resized))
        } else {
            (width, height, image::DynamicImage::ImageRgba8(rgba_image))
        };

        let rgb_image = dynamic_img.into_rgb8();

        // In-memory JPEG compression
        let mut jpeg_bytes = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 80);
            encoder.encode(
                rgb_image.as_raw(),
                target_w,
                target_h,
                image::ExtendedColorType::Rgb8,
            )?;
        }

        // Section 5: Discard if the resulting image exceeds 2 MB
        if jpeg_bytes.len() > 2 * 1024 * 1024 {
            tracing::warn!(
                "Screenshot discarded: compressed size exceeds 2MB limit ({} bytes)",
                jpeg_bytes.len()
            );
            return Ok(None);
        }

        // RF-AGENT-02-5 / DP-05: Fire native desktop notification to worker
        NotificationDispatcher::notify_screenshot_taken();

        let image_b64 = STANDARD.encode(&jpeg_bytes);
        tracing::info!(
            "Screenshot captured and scheduled for encryption ({}x{}, {} bytes)",
            target_w,
            target_h,
            jpeg_bytes.len()
        );

        Ok(Some(ScreenshotItem {
            ts: Utc::now(),
            format: "jpeg".to_string(),
            width: target_w,
            height: target_h,
            display_index: 0,
            image_b64,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_collector_headless_resilient() {
        // In headless CI or environments without display permissions, capture should gracefully return Ok(None) or Ok(Some)
        let result = ScreenshotCollector::capture_primary();
        assert!(result.is_ok());

        if let Ok(Some(item)) = result {
            assert!(item.width <= 1920);
            assert!(item.height <= 1080);
            assert_eq!(item.format, "jpeg");
            assert!(!item.image_b64.is_empty());
        }
    }
}
