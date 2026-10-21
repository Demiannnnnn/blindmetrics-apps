use crate::error::CommandError;
use telemetry_crypto_core::EnrollmentQr;

pub const MAX_QR_IMAGE_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

/// Decodes an enrollment QR code from raw image bytes (PNG, JPEG, WebP).
///
/// Limits defensive parsing to avoid decompression bombs and excessive memory consumption.
/// Validates that exactly one QR code is present and that its payload is a valid enrollment package.
///
/// # Errors
/// - `EMPTY_IMAGE` if `bytes` is empty.
/// - `IMAGE_TOO_LARGE` if `bytes.len()` exceeds 10 MiB.
/// - `INVALID_IMAGE` if decoding the image fails or exceeds decompression limits.
/// - `QR_NOT_FOUND` if no QR code could be detected/decoded.
/// - `QR_MULTIPLE_FOUND` if more than one QR grid is detected in the image.
/// - `INVALID_QR_PAYLOAD` if the QR text is not a valid `EnrollmentQr` payload.
pub fn decode_qr_from_image_bytes(bytes: &[u8]) -> Result<String, CommandError> {
    if bytes.is_empty() {
        return Err(CommandError::new(
            "EMPTY_IMAGE",
            "La imagen seleccionada está vacía.",
        ));
    }

    if bytes.len() > MAX_QR_IMAGE_BYTES {
        return Err(CommandError::new(
            "IMAGE_TOO_LARGE",
            "La imagen supera el tamaño máximo permitido de 10 MiB.",
        ));
    }

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);

    let cursor = std::io::Cursor::new(bytes);
    let mut reader = image::ImageReader::new(cursor)
        .with_guessed_format()
        .map_err(|e| {
            CommandError::new(
                "INVALID_IMAGE",
                format!("Formato de imagen inválido o no reconocido: {}", e),
            )
        })?;
    reader.limits(limits);

    let dynamic_img = reader.decode().map_err(|e| {
        CommandError::new(
            "INVALID_IMAGE",
            format!("No se pudo decodificar la imagen: {}", e),
        )
    })?;

    let luma = dynamic_img.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(luma);
    let grids = prepared.detect_grids();

    if grids.is_empty() {
        return Err(CommandError::new(
            "QR_NOT_FOUND",
            "No se encontró ningún código QR en la imagen seleccionada.",
        ));
    }

    if grids.len() > 1 {
        return Err(CommandError::new(
            "QR_MULTIPLE_FOUND",
            "La imagen contiene más de un código QR. Usa una imagen con un único código de vinculación.",
        ));
    }

    let (_meta, raw_text) = match grids[0].decode() {
        Ok(res) => res,
        Err(_) => {
            return Err(CommandError::new(
                "QR_NOT_FOUND",
                "No se encontró ningún código QR en la imagen seleccionada.",
            ));
        }
    };

    let qr = EnrollmentQr::parse_payload(&raw_text).map_err(|_| {
        CommandError::new(
            "INVALID_QR_PAYLOAD",
            "El código QR de la imagen no es un código de vinculación válido.",
        )
    })?;

    qr.to_payload().map_err(|e| {
        CommandError::new(
            "INVALID_QR_PAYLOAD",
            format!("Error al serializar el payload de vinculación: {}", e),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_qr_image(content: &str, format: image::ImageFormat) -> Vec<u8> {
        let code = qrcode::QrCode::new(content.as_bytes()).expect("qr code generation");
        let img: image::RgbImage = code
            .render::<image::Rgb<u8>>()
            .min_dimensions(300, 300)
            .build();
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), format)
            .expect("image serialization");
        bytes
    }

    #[test]
    fn test_qr_decode_valid_png_and_jpeg_enrollment_payload() {
        let qr = EnrollmentQr::new(
            "https://api.telemetry.local/api/v1".into(),
            "A68K-2PZS-PC4H".into(),
            "a".repeat(64),
        )
        .expect("enrollment qr");
        let payload = qr.to_payload().expect("to_payload");

        // Test PNG
        let png_bytes = generate_qr_image(&payload, image::ImageFormat::Png);
        let decoded_png = decode_qr_from_image_bytes(&png_bytes).expect("decode png");
        assert_eq!(decoded_png, payload);
        let parsed_png = EnrollmentQr::parse_payload(&decoded_png).expect("parse payload png");
        assert_eq!(parsed_png, qr);

        // Test JPEG
        let jpeg_bytes = generate_qr_image(&payload, image::ImageFormat::Jpeg);
        let decoded_jpeg = decode_qr_from_image_bytes(&jpeg_bytes).expect("decode jpeg");
        assert_eq!(decoded_jpeg, payload);
        let parsed_jpeg = EnrollmentQr::parse_payload(&decoded_jpeg).expect("parse payload jpeg");
        assert_eq!(parsed_jpeg, qr);
    }

    #[test]
    fn test_qr_decode_blank_image_returns_qr_not_found() {
        let img = image::RgbImage::from_pixel(300, 300, image::Rgb([255, 255, 255]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("png write");

        let err = decode_qr_from_image_bytes(&bytes).expect_err("should fail");
        assert_eq!(err.code, "QR_NOT_FOUND");
    }

    #[test]
    fn test_qr_decode_non_enrollment_qr_returns_invalid_qr_payload() {
        let png_bytes = generate_qr_image("https://example.com", image::ImageFormat::Png);
        let err = decode_qr_from_image_bytes(&png_bytes).expect_err("should fail");
        assert_eq!(err.code, "INVALID_QR_PAYLOAD");
    }

    #[test]
    fn test_qr_decode_multiple_qrs_returns_multiple_found() {
        let code1 = qrcode::QrCode::new(b"https://example.com/first").expect("qr1");
        let img1: image::RgbImage = code1
            .render::<image::Rgb<u8>>()
            .min_dimensions(200, 200)
            .build();

        let code2 = qrcode::QrCode::new(b"https://example.com/second").expect("qr2");
        let img2: image::RgbImage = code2
            .render::<image::Rgb<u8>>()
            .min_dimensions(200, 200)
            .build();

        let w = img1.width() + img2.width() + 40;
        let h = img1.height().max(img2.height()) + 40;
        let mut composite = image::RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));

        image::imageops::overlay(&mut composite, &img1, 10, 10);
        image::imageops::overlay(&mut composite, &img2, (img1.width() + 20) as i64, 10);

        let mut bytes = Vec::new();
        composite
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("composite write");

        let err = decode_qr_from_image_bytes(&bytes).expect_err("should fail");
        assert_eq!(err.code, "QR_MULTIPLE_FOUND");
    }

    #[test]
    fn test_qr_decode_rejects_empty_corrupt_and_oversized_bytes() {
        let empty_err = decode_qr_from_image_bytes(&[]).expect_err("empty should fail");
        assert_eq!(empty_err.code, "EMPTY_IMAGE");

        let corrupt_err =
            decode_qr_from_image_bytes(&[1, 2, 3, 4, 5, 6, 7, 8]).expect_err("corrupt should fail");
        assert_eq!(corrupt_err.code, "INVALID_IMAGE");

        let oversized = vec![0u8; MAX_QR_IMAGE_BYTES + 1];
        let oversized_err =
            decode_qr_from_image_bytes(&oversized).expect_err("oversized should fail");
        assert_eq!(oversized_err.code, "IMAGE_TOO_LARGE");
    }
}
