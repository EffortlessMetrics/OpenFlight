// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virpil button matrix resolver.
//!
//! VIRPIL VPC devices use a button matrix internally: physical switch positions
//! are addressed by (row, column) pairs within the matrix. This module maps
//! those physical positions to logical button IDs that appear in HID reports.
//!
//! # Shift layers
//!
//! VIRPIL firmware supports shifted / layered buttons. When a designated shift
//! button is held, the same physical switch produces a different logical ID.
//! The [`ButtonMatrix`] supports up to 4 shift layers (0 = base, 1–3 = shifted).
//!
//! # Example
//!
//! ```
//! use flight_hotas_virpil::button_matrix::ButtonMatrix;
//!
//! let matrix = ButtonMatrix::new(8, 16, 2);
//! assert_eq!(matrix.resolve(0, 0), Some(1));
//! assert_eq!(matrix.resolve(0, 1), Some(2));
//! assert_eq!(matrix.resolve_shifted(0, 0, 1), Some(129));
//! ```

use thiserror::Error;

/// Maximum number of shift layers supported (base + 3 shifted).
pub const MAX_SHIFT_LAYERS: u8 = 4;

/// Error returned when building or querying a [`ButtonMatrix`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ButtonMatrixError {
    #[error("row {row} out of range (max {max_rows})")]
    RowOutOfRange { row: u8, max_rows: u8 },
    #[error("column {col} out of range (max {max_cols})")]
    ColOutOfRange { col: u8, max_cols: u8 },
    #[error("shift layer {layer} out of range (max {max_layers})")]
    LayerOutOfRange { layer: u8, max_layers: u8 },
    #[error("matrix dimensions too large: {rows}×{cols}×{layers} exceeds u8 button space")]
    DimensionsTooLarge { rows: u8, cols: u8, layers: u8 },
}

/// A physical-to-logical button matrix resolver for VIRPIL VPC devices.
///
/// The matrix maps `(row, col)` positions to 1-indexed logical button IDs.
/// Shift layers multiply the button space: layer 0 produces buttons
/// `1..=rows*cols`, layer 1 produces `rows*cols+1..=2*rows*cols`, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonMatrix {
    rows: u8,
    cols: u8,
    shift_layers: u8,
    buttons_per_layer: u16,
}

impl ButtonMatrix {
    /// Create a new button matrix.
    ///
    /// # Arguments
    ///
    /// * `rows` — number of physical rows in the matrix (must be ≥ 1).
    /// * `cols` — number of physical columns in the matrix (must be ≥ 1).
    /// * `shift_layers` — total number of layers including base (1–4).
    ///
    /// # Errors
    ///
    /// Returns [`ButtonMatrixError::DimensionsTooLarge`] if the total button
    /// count (`rows × cols × layers`) exceeds 255.
    pub fn new(rows: u8, cols: u8, shift_layers: u8) -> Self {
        let layers = shift_layers.clamp(1, MAX_SHIFT_LAYERS);
        let buttons_per_layer = rows as u16 * cols as u16;
        Self {
            rows,
            cols,
            shift_layers: layers,
            buttons_per_layer,
        }
    }

    /// Validate that the matrix dimensions fit within the button ID space.
    pub fn validate(&self) -> Result<(), ButtonMatrixError> {
        let total = self.buttons_per_layer as u32 * self.shift_layers as u32;
        if total > 255 {
            return Err(ButtonMatrixError::DimensionsTooLarge {
                rows: self.rows,
                cols: self.cols,
                layers: self.shift_layers,
            });
        }
        Ok(())
    }

    /// Number of rows in the matrix.
    pub fn rows(&self) -> u8 {
        self.rows
    }

    /// Number of columns in the matrix.
    pub fn cols(&self) -> u8 {
        self.cols
    }

    /// Number of shift layers (including the base layer).
    pub fn shift_layers(&self) -> u8 {
        self.shift_layers
    }

    /// Total number of logical buttons across all layers.
    pub fn total_buttons(&self) -> u16 {
        self.buttons_per_layer * self.shift_layers as u16
    }

    /// Resolve a physical `(row, col)` position to a 1-indexed logical button ID
    /// on the base layer (layer 0).
    ///
    /// Returns `None` if the position is out of range.
    pub fn resolve(&self, row: u8, col: u8) -> Option<u8> {
        self.resolve_shifted(row, col, 0)
    }

    /// Resolve a physical `(row, col)` position to a 1-indexed logical button ID
    /// on the given shift layer.
    ///
    /// Layer 0 is the base (unshifted) layer. Returns `None` if any argument is
    /// out of range or the resulting button ID exceeds `u8::MAX`.
    pub fn resolve_shifted(&self, row: u8, col: u8, shift_layer: u8) -> Option<u8> {
        if row >= self.rows || col >= self.cols || shift_layer >= self.shift_layers {
            return None;
        }
        let offset = shift_layer as u16 * self.buttons_per_layer;
        let index = row as u16 * self.cols as u16 + col as u16;
        let button_id = offset + index + 1; // 1-indexed
        u8::try_from(button_id).ok()
    }

    /// Reverse-map a 1-indexed logical button ID back to `(row, col, layer)`.
    ///
    /// Returns `None` for button IDs outside the matrix range.
    pub fn reverse(&self, button_id: u8) -> Option<(u8, u8, u8)> {
        if button_id == 0 {
            return None;
        }
        let id = button_id as u16 - 1;
        let layer = (id / self.buttons_per_layer) as u8;
        if layer >= self.shift_layers {
            return None;
        }
        let within_layer = (id % self.buttons_per_layer) as u8;
        let row = within_layer / self.cols;
        let col = within_layer % self.cols;
        Some((row, col, layer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_resolve() {
        let m = ButtonMatrix::new(4, 4, 1);
        assert_eq!(m.resolve(0, 0), Some(1));
        assert_eq!(m.resolve(0, 3), Some(4));
        assert_eq!(m.resolve(1, 0), Some(5));
        assert_eq!(m.resolve(3, 3), Some(16));
    }

    #[test]
    fn out_of_range_returns_none() {
        let m = ButtonMatrix::new(4, 4, 1);
        assert_eq!(m.resolve(4, 0), None);
        assert_eq!(m.resolve(0, 4), None);
        assert_eq!(m.resolve(255, 255), None);
    }

    #[test]
    fn shift_layers() {
        let m = ButtonMatrix::new(8, 16, 2);
        // Base layer: buttons 1..=128
        assert_eq!(m.resolve(0, 0), Some(1));
        assert_eq!(m.resolve(7, 15), Some(128));
        // Shift layer 1: buttons 129..=256 (but capped at u8::MAX=255)
        assert_eq!(m.resolve_shifted(0, 0, 1), Some(129));
    }

    #[test]
    fn shift_layer_out_of_range() {
        let m = ButtonMatrix::new(4, 4, 2);
        assert_eq!(m.resolve_shifted(0, 0, 2), None);
        assert_eq!(m.resolve_shifted(0, 0, 255), None);
    }

    #[test]
    fn max_shift_layers_clamped() {
        let m = ButtonMatrix::new(2, 2, 10);
        assert_eq!(m.shift_layers(), MAX_SHIFT_LAYERS);
    }

    #[test]
    fn reverse_basic() {
        let m = ButtonMatrix::new(4, 4, 1);
        assert_eq!(m.reverse(1), Some((0, 0, 0)));
        assert_eq!(m.reverse(4), Some((0, 3, 0)));
        assert_eq!(m.reverse(5), Some((1, 0, 0)));
        assert_eq!(m.reverse(16), Some((3, 3, 0)));
    }

    #[test]
    fn reverse_with_layers() {
        let m = ButtonMatrix::new(4, 4, 2);
        assert_eq!(m.reverse(1), Some((0, 0, 0)));
        assert_eq!(m.reverse(16), Some((3, 3, 0)));
        assert_eq!(m.reverse(17), Some((0, 0, 1)));
        assert_eq!(m.reverse(32), Some((3, 3, 1)));
    }

    #[test]
    fn reverse_zero_returns_none() {
        let m = ButtonMatrix::new(4, 4, 1);
        assert_eq!(m.reverse(0), None);
    }

    #[test]
    fn reverse_out_of_range_returns_none() {
        let m = ButtonMatrix::new(4, 4, 1);
        assert_eq!(m.reverse(17), None);
    }

    #[test]
    fn roundtrip_resolve_reverse() {
        let m = ButtonMatrix::new(8, 16, 2);
        for layer in 0..m.shift_layers() {
            for row in 0..m.rows() {
                for col in 0..m.cols() {
                    if let Some(btn) = m.resolve_shifted(row, col, layer) {
                        let (r, c, l) = m.reverse(btn).unwrap();
                        assert_eq!((r, c, l), (row, col, layer), "roundtrip failed for btn {btn}");
                    }
                }
            }
        }
    }

    #[test]
    fn total_buttons() {
        let m = ButtonMatrix::new(4, 4, 2);
        assert_eq!(m.total_buttons(), 32);
    }

    #[test]
    fn single_layer_matrix() {
        let m = ButtonMatrix::new(1, 1, 1);
        assert_eq!(m.resolve(0, 0), Some(1));
        assert_eq!(m.total_buttons(), 1);
    }

    #[test]
    fn validate_ok() {
        let m = ButtonMatrix::new(4, 4, 2);
        assert!(m.validate().is_ok());
    }

    #[test]
    fn validate_too_large() {
        let m = ButtonMatrix::new(16, 16, 2);
        assert!(m.validate().is_err());
    }

    #[test]
    fn overflow_returns_none() {
        // 15×15×1 = 225 buttons, all fit in u8
        let m = ButtonMatrix::new(15, 15, 1);
        assert_eq!(m.resolve(14, 14), Some(225));
        // But with 2 layers, button 226+ would exceed for some, while
        // the second layer starts at 226 which fits in u8 up to 255
        let m2 = ButtonMatrix::new(15, 15, 2);
        // Layer 1, row 0, col 0 → 225 + 1 = 226
        assert_eq!(m2.resolve_shifted(0, 0, 1), Some(226));
        // Layer 1, row 1, col 14 → 225 + 30 = 255
        assert_eq!(m2.resolve_shifted(1, 14, 1), Some(255));
        // Layer 1, row 2, col 0 → 225 + 31 = 256 → overflows u8
        assert_eq!(m2.resolve_shifted(2, 0, 1), None);
    }

    #[test]
    fn accessors() {
        let m = ButtonMatrix::new(8, 16, 3);
        assert_eq!(m.rows(), 8);
        assert_eq!(m.cols(), 16);
        assert_eq!(m.shift_layers(), 3);
    }
}
