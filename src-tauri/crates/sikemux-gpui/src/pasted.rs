//! Pictures pasted from the clipboard, in a format every agent reads. macOS often hands
//! a copied picture over as TIFF only, which agents cannot open.

use gpui_kit::{Image, ImageFormat};

/// `image` as it is when agents read its format, else as a PNG; as it is when it does not
/// decode.
pub fn readable(image: Image) -> Image {
    if agents_read(image.format) {
        return image;
    }
    match png(&image.bytes) {
        Some(bytes) => Image::from_bytes(ImageFormat::Png, bytes),
        None => image,
    }
}

fn agents_read(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp | ImageFormat::Gif
    )
}

fn png(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    image::load_from_memory(bytes)
        .ok()?
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tiff_from_the_clipboard_is_kept_as_a_png() {
        let mut tiff = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
            .write_to(
                &mut std::io::Cursor::new(&mut tiff),
                image::ImageFormat::Tiff,
            )
            .expect("encode");
        let kept = readable(Image::from_bytes(ImageFormat::Tiff, tiff));
        assert_eq!(kept.format, ImageFormat::Png);
        let decoded = image::load_from_memory(&kept.bytes).expect("decode");
        assert_eq!((decoded.width(), decoded.height()), (3, 2));
    }
}
