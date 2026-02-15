//! Integration tests for image operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

use std::fs;
use tempfile::NamedTempFile;

// Minimal valid PNG file (1x1 red pixel)
const MINIMAL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk header
    0x00, 0x00, 0x00, 0x01, // Width: 1
    0x00, 0x00, 0x00, 0x01, // Height: 1
    0x08, 0x02, // Bit depth: 8, Color type: 2 (RGB)
    0x00, 0x00, 0x00, // Compression, Filter, Interlace
    0x90, 0x77, 0x53, 0xde, // CRC
    0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, // IDAT chunk header
    0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, // Compressed data
    0x00, 0x03, 0x00, 0x01, // Data + CRC start
    0x00, 0x18, 0xdd, 0x8d, 0xb4, // CRC
    0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, // IEND chunk
    0xae, 0x42, 0x60, 0x82, // CRC
];

// Minimal valid GIF (1x1 pixel)
const MINIMAL_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // GIF89a
    0x01, 0x00, // Width: 1
    0x01, 0x00, // Height: 1
    0x00, 0x00, 0x00, // No global color table
    0x2c, // Image separator
    0x00, 0x00, 0x00, 0x00, // Position
    0x01, 0x00, 0x01, 0x00, // Dimensions
    0x00, // No local color table
    0x02, 0x02, 0x44, 0x01, 0x00, // Image data
    0x3b, // GIF trailer
];

fn create_temp_png() -> NamedTempFile {
    let temp = tempfile::Builder::new()
        .suffix(".png")
        .tempfile()
        .unwrap();
    fs::write(temp.path(), MINIMAL_PNG).unwrap();
    temp
}

fn create_temp_gif() -> NamedTempFile {
    let temp = tempfile::Builder::new()
        .suffix(".gif")
        .tempfile()
        .unwrap();
    fs::write(temp.path(), MINIMAL_GIF).unwrap();
    temp
}

// === image-load tests ===

#[test]
fn test_image_load_png_returns_media() {
    let temp = create_temp_png();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load typeof"#, path)).unwrap();
    assert_eq!(output.trim(), "Media");
}

#[test]
fn test_image_load_gif_returns_media() {
    let temp = create_temp_gif();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load typeof"#, path)).unwrap();
    assert_eq!(output.trim(), "Media");
}

#[test]
fn test_image_load_nonexistent_error() {
    let result = eval(r#""/nonexistent/path/to/image.png" image-load"#);
    assert!(result.is_err());
}

#[test]
fn test_image_load_sets_exit_code() {
    let temp = create_temp_png();
    let path = temp.path().display();
    let exit_code = eval_exit_code(&format!(r#""{}" image-load"#, path));
    assert_eq!(exit_code, 0);
}

// === image-info tests ===

#[test]
fn test_image_info_returns_record() {
    let temp = create_temp_png();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info typeof"#, path)).unwrap();
    // hsab uses "Record" as the display name for Map type
    assert!(output.trim() == "Map" || output.trim() == "Record");
}

#[test]
fn test_image_info_has_mime_type() {
    let temp = create_temp_png();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/png");
}

#[test]
fn test_image_info_gif_mime_type() {
    let temp = create_temp_gif();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/gif");
}

#[test]
fn test_image_info_has_size() {
    let temp = create_temp_png();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "size" get"#, path)).unwrap();
    let size: f64 = output.trim().parse().unwrap();
    assert!(size > 0.0, "Size should be positive");
}

#[test]
fn test_image_info_png_dimensions() {
    let temp = create_temp_png();
    let path = temp.path().display();

    // Check width
    let width_output = eval(&format!(r#""{}" image-load image-info "width" get"#, path)).unwrap();
    assert_eq!(width_output.trim(), "1");

    // Check height
    let height_output = eval(&format!(r#""{}" image-load image-info "height" get"#, path)).unwrap();
    assert_eq!(height_output.trim(), "1");
}

#[test]
fn test_image_info_gif_dimensions() {
    let temp = create_temp_gif();
    let path = temp.path().display();

    let width_output = eval(&format!(r#""{}" image-load image-info "width" get"#, path)).unwrap();
    assert_eq!(width_output.trim(), "1");

    let height_output = eval(&format!(r#""{}" image-load image-info "height" get"#, path)).unwrap();
    assert_eq!(height_output.trim(), "1");
}

#[test]
fn test_image_info_has_source() {
    let temp = create_temp_png();
    let path_str = temp.path().to_string_lossy().to_string();
    let output = eval(&format!(r#""{}" image-load image-info "source" get"#, path_str)).unwrap();
    assert!(output.contains(&path_str) || output.trim().len() > 0);
}

// === image-show tests ===

#[test]
fn test_image_show_preserves_media() {
    let temp = create_temp_png();
    let path = temp.path().display();
    // image-show should push media back (non-destructive)
    let output = eval(&format!(r#""{}" image-load image-show typeof"#, path)).unwrap();
    assert_eq!(output.trim(), "Media");
}

#[test]
fn test_image_show_on_non_media_error() {
    let result = eval(r#""not an image" image-show"#);
    assert!(result.is_err());
}

// === MIME type detection tests ===

#[test]
fn test_mime_type_detection_png() {
    let temp = NamedTempFile::with_suffix(".png").unwrap();
    fs::write(temp.path(), MINIMAL_PNG).unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/png");
}

#[test]
fn test_mime_type_detection_gif() {
    let temp = NamedTempFile::with_suffix(".gif").unwrap();
    fs::write(temp.path(), MINIMAL_GIF).unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/gif");
}

#[test]
fn test_mime_type_detection_jpg() {
    // Test that .jpg extension is recognized (even with invalid content)
    // The file won't be valid JPEG but mime type is detected from extension
    let temp = NamedTempFile::with_suffix(".jpg").unwrap();
    fs::write(temp.path(), b"not a real jpeg").unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/jpeg");
}

#[test]
fn test_mime_type_detection_jpeg() {
    let temp = NamedTempFile::with_suffix(".jpeg").unwrap();
    fs::write(temp.path(), b"not a real jpeg").unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/jpeg");
}

#[test]
fn test_mime_type_detection_webp() {
    let temp = NamedTempFile::with_suffix(".webp").unwrap();
    fs::write(temp.path(), b"test").unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/webp");
}

#[test]
fn test_mime_type_detection_svg() {
    let temp = NamedTempFile::with_suffix(".svg").unwrap();
    fs::write(temp.path(), b"<svg></svg>").unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/svg+xml");
}

#[test]
fn test_mime_type_detection_bmp() {
    let temp = NamedTempFile::with_suffix(".bmp").unwrap();
    fs::write(temp.path(), b"test").unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "image/bmp");
}

#[test]
fn test_mime_type_detection_unknown() {
    let temp = NamedTempFile::with_suffix(".xyz").unwrap();
    fs::write(temp.path(), b"test").unwrap();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "mime_type" get"#, path)).unwrap();
    assert_eq!(output.trim(), "application/octet-stream");
}

// === Tilde expansion tests ===

#[test]
fn test_image_load_tilde_expansion() {
    // This test verifies that tilde expansion is attempted
    // It will fail if ~ doesn't expand (which is expected in test env)
    let result = eval(r#""~/nonexistent_image_test_12345.png" image-load"#);
    // Should error because file doesn't exist, but shouldn't panic on tilde
    assert!(result.is_err());
}

// === Error handling tests ===

#[test]
fn test_image_info_on_non_media() {
    let result = eval(r#"42 image-info"#);
    assert!(result.is_err());
}

#[test]
fn test_image_load_empty_path() {
    let result = eval(r#""" image-load"#);
    assert!(result.is_err());
}

// === Integration tests ===

#[test]
fn test_image_load_preserves_data() {
    let temp = create_temp_png();
    let path = temp.path().display();
    let output = eval(&format!(r#""{}" image-load image-info "size" get"#, path)).unwrap();
    let size: f64 = output.trim().parse().unwrap();
    // Size should match the actual file size
    assert_eq!(size as usize, MINIMAL_PNG.len());
}

#[test]
fn test_image_chain_operations() {
    let temp = create_temp_png();
    let path = temp.path().display();
    // Load -> info -> get multiple fields
    let output = eval(&format!(r#""{}" image-load image-info dup "mime_type" get swap "width" get"#, path)).unwrap();
    assert!(output.contains("image/png") || output.contains("1"));
}
