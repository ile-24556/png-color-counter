use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::Cursor;
use std::io::prelude::*;
use std::path::Path;

use anyhow::{Result, bail, ensure};
use libdeflater::Decompressor;

pub fn count_colors(path: &Path) -> Result<usize> {
    let crc_table = make_crc_table();

    let mut cur = Cursor::new(fs::read(path)?);

    const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let signature = {
        let mut buf = [0_u8; 8];
        cur.read_exact(&mut buf)?;
        buf
    };
    ensure!(signature == PNG_SIGNATURE, "wrong signature: {signature:?}");

    let ihdr = {
        let ihdr = Chunk::new(&mut cur, &crc_table)?;
        ensure!(
            ihdr.data.len() == 13 && ihdr.type_ == *b"IHDR",
            "wrong IHDR chunk: {:?}",
            ihdr
        );
        Ihdr::new(&ihdr.data)?
    };

    let compressed_data = {
        let mut data = Vec::new();
        loop {
            let chunk = Chunk::new(&mut cur, &crc_table)?;
            match &chunk.type_ {
                b"IDAT" => data.extend(chunk.data),
                b"IEND" => break,
                _ => continue,
            };
        }
        data
    };

    let bytes_per_pixel: usize = match ihdr.color_type {
        ColorType::Truecolor => 3,
        ColorType::TruecolorWithAlpha => 4,
    };

    let filtered_data = {
        let mut decompressor = Decompressor::new();
        let expected_len = (1 + bytes_per_pixel * ihdr.width) * ihdr.height;
        let mut filtered_data = vec![0; expected_len];
        let len = decompressor.zlib_decompress(&compressed_data, &mut filtered_data)?;
        ensure!(len == expected_len, "unexpected length of filtered data",);
        filtered_data
    };

    let raw_data = {
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
        raw_data
    };

    let color_count = {
        let mut raw_data = Cursor::new(raw_data);
        match ihdr.color_type {
            ColorType::Truecolor => {
                let mut colors = HashSet::new();
                let mut buf = [0; 3];
                while raw_data.read_exact(&mut buf).is_ok() {
                    colors.insert(buf);
                }
                colors.len()
            }
            ColorType::TruecolorWithAlpha => {
                let mut colors = HashSet::new();
                let mut buf = [0; 4];
                while raw_data.read_exact(&mut buf).is_ok() {
                    colors.insert(buf);
                }
                colors.len()
            }
        }
    };

    Ok(color_count)
}

struct Chunk {
    type_: [u8; 4],
    data: Vec<u8>,
}

impl Chunk {
    fn new(cur: &mut Cursor<Vec<u8>>, crc_table: &[u32; 256]) -> Result<Chunk> {
        let length = u32::from_be_bytes(read_four_bytes(cur)?);
        let type_ = read_four_bytes(cur)?;
        let data = {
            let mut buf = vec![0_u8; length as usize];
            cur.read_exact(&mut buf)?;
            buf
        };
        let crc = u32::from_be_bytes(read_four_bytes(cur)?);
        ensure!(
            crc == Chunk::calculate_crc(&crc_table, &type_, &data),
            "wrong CRC"
        );
        Ok(Chunk { type_, data })
    }

    fn type_name(&self) -> &str {
        std::str::from_utf8(&self.type_).expect("should decode type")
    }

    /// Formats chunk data into readable representation.
    fn format_data(&self, max_bytes: usize) -> String {
        format!(
            "[{}{}]",
            self.data
                .iter()
                .take(max_bytes)
                .map(|x| format!("{x:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
            if self.data.len() > max_bytes {
                " ..."
            } else {
                ""
            }
        )
    }

    fn calculate_crc(crc_table: &[u32; 256], chunk_type: &[u8; 4], chunk_data: &[u8]) -> u32 {
        chunk_type
            .iter()
            .chain(chunk_data)
            .fold(0xffff_ffff, |c, &byte| {
                crc_table[(c ^ (byte as u32)) as usize & 0xff] ^ (c >> 8)
            })
            ^ 0xffff_ffff
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}  {:>4} B: {}",
            self.type_name(),
            self.data.len(),
            self.format_data(16),
        )
    }
}

struct Ihdr {
    width: usize,
    height: usize,
    bit_depth: u8,
    color_type: ColorType,
    compression_method: u8,
    filter_method: u8,
    interlace_method: u8,
}

impl Ihdr {
    fn new(data: &[u8]) -> Result<Ihdr> {
        let mut cur = Cursor::new(data.to_vec());
        let ihdr = Ihdr {
            width: u32::from_be_bytes(read_four_bytes(&mut cur)?) as usize,
            height: u32::from_be_bytes(read_four_bytes(&mut cur)?) as usize,
            bit_depth: data[8],
            color_type: match data[9] {
                2 => ColorType::Truecolor,
                6 => ColorType::TruecolorWithAlpha,
                x => bail!("unsupported color type: {}", x),
            },
            compression_method: data[10],
            filter_method: data[11],
            interlace_method: data[12],
        };

        ensure!(
            ihdr.bit_depth == 8,
            "unsupported bit depth: {}",
            ihdr.bit_depth
        );
        ensure!(
            ihdr.compression_method == 0,
            "unknown compression method: {}",
            ihdr.compression_method
        );
        ensure!(
            ihdr.filter_method == 0,
            "unknown filter method: {}",
            ihdr.filter_method
        );
        ensure!(
            ihdr.interlace_method == 0,
            "interlace image is not supported"
        );

        Ok(ihdr)
    }
}

/// Makes a CRC table.
fn make_crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    for i in 0..256 {
        let mut c: u32 = i;
        for _ in 0..8 {
            if c & 1 == 1 {
                c = 0xedb8_8320 ^ (c >> 1);
            } else {
                c >>= 1
            }
        }
        table[i as usize] = c;
    }
    table
}

/// Reads 4 bytes from cursor and returns the buffer.
fn read_four_bytes(cur: &mut Cursor<Vec<u8>>) -> Result<[u8; 4]> {
    let mut buf = [0_u8; 4];
    cur.read_exact(&mut buf)?;
    Ok(buf)
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

enum ColorType {
    Truecolor,
    TruecolorWithAlpha,
}
