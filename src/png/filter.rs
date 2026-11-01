use anyhow::{Result, bail};

use crate::png::Ihdr;

pub fn unfilter(filtered_data: &[u8], ihdr: &Ihdr) -> Result<Vec<u8>> {
    let bytes_per_pixel = ihdr.color_type.bytes_per_pixel();
    let raw_width = ihdr.width * bytes_per_pixel;
    let mut raw_data = vec![0u8; ihdr.height * raw_width];

    for row in 0..ihdr.height {
        for pos_x in 0..raw_width {
            let current = filtered_data[(1 + raw_width) * row + 1 + pos_x];

            let a = if pos_x >= bytes_per_pixel {
                raw_data[raw_width * row + pos_x - bytes_per_pixel]
            } else {
                0
            };
            let b = if row > 0 {
                raw_data[raw_width * (row - 1) + pos_x]
            } else {
                0
            };
            let c = if pos_x >= bytes_per_pixel && row > 0 {
                raw_data[raw_width * (row - 1) + pos_x - bytes_per_pixel]
            } else {
                0
            };

            let raw_value = match filtered_data[(1 + raw_width) * row] {
                0 => current,
                1 => current.wrapping_add(a),
                2 => current.wrapping_add(b),
                3 => current.wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => current.wrapping_add(paeth_predictor(a, b, c)),
                f => bail!("unexpected filter type: {f}"),
            };
            raw_data[raw_width * row + pos_x] = raw_value;
        }
    }
    Ok(raw_data)
}

/// The PaethPredictor function.
/// ```text
/// c b
/// a x
/// ```
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}
