#!/usr/bin/env cargo +nightly -Zscript
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generate placeholder BMP images for the WiX installer.
//!
//! Usage:
//!   cargo +nightly -Zscript installer/wix/generate-images.rs
//!
//! The script writes `banner.bmp` (493x58) and `dialog.bmp` (374x316) in the
//! current directory, matching the historical generator behavior.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

const PRIMARY_BLUE: Rgb = Rgb::new(41, 98, 255);
const DARK_BLUE: Rgb = Rgb::new(26, 62, 161);

#[derive(Clone, Copy)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

fn main() -> io::Result<()> {
    create_bmp(493, 58, "banner.bmp", PRIMARY_BLUE)?;
    create_bmp(374, 316, "dialog.bmp", DARK_BLUE)?;

    println!("\nPlaceholder images created successfully.");
    println!("For a professional installer, replace these with proper branded images:");
    println!("  - banner.bmp: 493x58 pixels, displayed at top of installer dialogs");
    println!("  - dialog.bmp: 374x316 pixels, displayed on welcome/finish pages");

    Ok(())
}

fn create_bmp(width: u32, height: u32, filename: &str, bg_color: Rgb) -> io::Result<()> {
    let row_size = (width * 3 + 3) & !3;
    let pixel_data_size = row_size * height;
    let file_size = 54 + pixel_data_size;

    let mut file = File::create(Path::new(filename))?;

    file.write_all(b"BM")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(&0u16.to_le_bytes())?;
    file.write_all(&0u16.to_le_bytes())?;
    file.write_all(&54u32.to_le_bytes())?;

    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&(width as i32).to_le_bytes())?;
    file.write_all(&(height as i32).to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&24u16.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&pixel_data_size.to_le_bytes())?;
    file.write_all(&2835i32.to_le_bytes())?;
    file.write_all(&2835i32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;

    let mut row = Vec::with_capacity(row_size as usize);
    for _ in 0..width {
        row.extend_from_slice(&[bg_color.blue, bg_color.green, bg_color.red]);
    }
    row.resize(row_size as usize, 0);

    for _ in 0..height {
        file.write_all(&row)?;
    }

    println!("Created {filename} ({width}x{height})");
    Ok(())
}
