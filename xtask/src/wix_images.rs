// SPDX-License-Identifier: MIT OR Apache-2.0

//! WiX installer image generation.
//!
//! Generates the placeholder BMP assets used by the Windows installer without
//! requiring Python or Pillow on developer machines.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const PRIMARY_BLUE: Rgb = Rgb(41, 98, 255);
const DARK_BLUE: Rgb = Rgb(26, 62, 161);

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

pub fn run_wix_generate_images(output_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    create_bmp(493, 58, &output_dir.join("banner.bmp"), PRIMARY_BLUE)?;
    create_bmp(374, 316, &output_dir.join("dialog.bmp"), DARK_BLUE)?;

    println!("\nPlaceholder images created successfully.");
    println!("For a professional installer, replace these with proper branded images:");
    println!("  - banner.bmp: 493x58 pixels, displayed at top of installer dialogs");
    println!("  - dialog.bmp: 374x316 pixels, displayed on welcome/finish pages");

    Ok(())
}

fn create_bmp(width: u32, height: u32, filename: &Path, bg_color: Rgb) -> Result<()> {
    let row_size = (width * 3).next_multiple_of(4);
    let pixel_data_size = row_size * height;
    let file_size = 54 + pixel_data_size;

    let file = File::create(filename)
        .with_context(|| format!("failed to create {}", filename.display()))?;
    let mut writer = BufWriter::new(file);

    writer.write_all(b"BM")?;
    writer.write_all(&file_size.to_le_bytes())?;
    writer.write_all(&0_u16.to_le_bytes())?;
    writer.write_all(&0_u16.to_le_bytes())?;
    writer.write_all(&54_u32.to_le_bytes())?;

    writer.write_all(&40_u32.to_le_bytes())?;
    writer.write_all(&(width as i32).to_le_bytes())?;
    writer.write_all(&(height as i32).to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?;
    writer.write_all(&24_u16.to_le_bytes())?;
    writer.write_all(&0_u32.to_le_bytes())?;
    writer.write_all(&pixel_data_size.to_le_bytes())?;
    writer.write_all(&2835_i32.to_le_bytes())?;
    writer.write_all(&2835_i32.to_le_bytes())?;
    writer.write_all(&0_u32.to_le_bytes())?;
    writer.write_all(&0_u32.to_le_bytes())?;

    let Rgb(r, g, b) = bg_color;
    let mut row = Vec::with_capacity(row_size as usize);
    for _ in 0..width {
        row.extend_from_slice(&[b, g, r]);
    }
    row.resize(row_size as usize, 0);

    for _ in 0..height {
        writer.write_all(&row)?;
    }
    writer.flush()?;

    println!("Created {} ({}x{})", filename.display(), width, height);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_valid_bmp_header() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bmp = temp_dir.path().join("test.bmp");

        create_bmp(2, 2, &bmp, Rgb(1, 2, 3)).unwrap();

        let bytes = std::fs::read(bmp).unwrap();
        assert_eq!(&bytes[..2], b"BM");
        assert_eq!(u32::from_le_bytes(bytes[10..14].try_into().unwrap()), 54);
        assert_eq!(u32::from_le_bytes(bytes[14..18].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(bytes[18..22].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(bytes[22..26].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(bytes[28..30].try_into().unwrap()), 24);
    }
}
