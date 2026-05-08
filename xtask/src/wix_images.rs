// SPDX-License-Identifier: MIT OR Apache-2.0

//! WiX installer bitmap generation.
//!
//! Keeps the installer placeholder artwork generation in Rust so the workspace
//! automation does not depend on Python for simple BMP asset regeneration.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

const PRIMARY_BLUE: (u8, u8, u8) = (41, 98, 255);
const DARK_BLUE: (u8, u8, u8) = (26, 62, 161);

pub fn run_wix_images() -> Result<()> {
    let wix_dir = Path::new("installer/wix");
    create_bmp(493, 58, &wix_dir.join("banner.bmp"), PRIMARY_BLUE)
        .context("failed to generate WiX banner bitmap")?;
    create_bmp(374, 316, &wix_dir.join("dialog.bmp"), DARK_BLUE)
        .context("failed to generate WiX dialog bitmap")?;

    println!("\nPlaceholder images created successfully.");
    println!("For a professional installer, replace these with proper branded images:");
    println!("  - installer/wix/banner.bmp: 493x58 pixels, displayed at top of installer dialogs");
    println!("  - installer/wix/dialog.bmp: 374x316 pixels, displayed on welcome/finish pages");
    Ok(())
}

fn create_bmp(width: u32, height: u32, path: &Path, bg_color: (u8, u8, u8)) -> Result<()> {
    let row_size = (width * 3).div_ceil(4) * 4;
    let pixel_data_size = row_size * height;
    let file_size = 54 + pixel_data_size;

    let mut file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;

    // BMP header (14 bytes).
    file.write_all(b"BM")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(&0_u16.to_le_bytes())?;
    file.write_all(&0_u16.to_le_bytes())?;
    file.write_all(&54_u32.to_le_bytes())?;

    // DIB header (BITMAPINFOHEADER, 40 bytes).
    file.write_all(&40_u32.to_le_bytes())?;
    file.write_all(&(width as i32).to_le_bytes())?;
    file.write_all(&(height as i32).to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&24_u16.to_le_bytes())?;
    file.write_all(&0_u32.to_le_bytes())?;
    file.write_all(&pixel_data_size.to_le_bytes())?;
    file.write_all(&2835_i32.to_le_bytes())?;
    file.write_all(&2835_i32.to_le_bytes())?;
    file.write_all(&0_u32.to_le_bytes())?;
    file.write_all(&0_u32.to_le_bytes())?;

    // Pixel data is BGR, bottom-to-top, with rows padded to a four-byte boundary.
    let (r, g, b) = bg_color;
    let mut row = Vec::with_capacity(row_size as usize);
    for _ in 0..width {
        row.extend_from_slice(&[b, g, r]);
    }
    row.resize(row_size as usize, 0);

    for _ in 0..height {
        file.write_all(&row)?;
    }

    println!("Created {} ({}x{})", path.display(), width, height);
    Ok(())
}
