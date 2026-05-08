// SPDX-License-Identifier: MIT OR Apache-2.0

//! WiX installer image generation.
//!
//! This replaces the old Python helper with a native Rust xtask command so
//! installer placeholder assets can be regenerated without Python tooling.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

const PRIMARY_BLUE: (u8, u8, u8) = (41, 98, 255);
const DARK_BLUE: (u8, u8, u8) = (26, 62, 161);

/// Regenerate the WiX placeholder BMP assets.
pub fn run_generate_wix_images() -> Result<()> {
    let out_dir = Path::new("installer/wix");
    create_bmp(493, 58, &out_dir.join("banner.bmp"), PRIMARY_BLUE)?;
    create_bmp(374, 316, &out_dir.join("dialog.bmp"), DARK_BLUE)?;

    println!("\nPlaceholder images created successfully.");
    println!("For a professional installer, replace these with proper branded images:");
    println!("  - banner.bmp: 493x58 pixels, displayed at top of installer dialogs");
    println!("  - dialog.bmp: 374x316 pixels, displayed on welcome/finish pages");
    Ok(())
}

fn create_bmp(width: i32, height: i32, filename: &Path, bg_color: (u8, u8, u8)) -> Result<()> {
    let width_u32 = u32::try_from(width).context("BMP width must be positive")?;
    let height_u32 = u32::try_from(height).context("BMP height must be positive")?;
    let row_size = (width_u32 * 3 + 3) & !3;
    let pixel_data_size = row_size * height_u32;
    let file_size = 54 + pixel_data_size;

    let mut file = File::create(filename)
        .with_context(|| format!("failed to create {}", filename.display()))?;

    // BMP file header.
    file.write_all(b"BM")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(&0u16.to_le_bytes())?;
    file.write_all(&0u16.to_le_bytes())?;
    file.write_all(&54u32.to_le_bytes())?;

    // DIB BITMAPINFOHEADER.
    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&width.to_le_bytes())?;
    file.write_all(&height.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&24u16.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&pixel_data_size.to_le_bytes())?;
    file.write_all(&2835i32.to_le_bytes())?;
    file.write_all(&2835i32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;

    let (r, g, b) = bg_color;
    let mut row = Vec::with_capacity(row_size as usize);
    for _ in 0..width_u32 {
        row.extend_from_slice(&[b, g, r]);
    }
    row.resize(row_size as usize, 0);

    for _ in 0..height_u32 {
        file.write_all(&row)?;
    }

    println!("Created {} ({}x{})", filename.display(), width, height);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_valid_bmp_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.bmp");
        create_bmp(2, 3, &path, (1, 2, 3)).unwrap();

        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(i32::from_le_bytes(bytes[18..22].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(bytes[22..26].try_into().unwrap()), 3);
        assert_eq!(u16::from_le_bytes(bytes[28..30].try_into().unwrap()), 24);
    }
}
