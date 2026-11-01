use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::io::prelude::*;
use std::path::Path;

use anyhow::{Result, ensure};

mod chunk;
mod deflate;
mod filter;

use chunk::{Chunk, ColorType, Ihdr};

pub fn count_colors(path: &Path) -> Result<usize> {
    let crc_table = chunk::make_crc_table();

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

    let filtered_data = deflate::decompress(&compressed_data, &ihdr)?;
    let raw_data = filter::unfilter(&filtered_data, &ihdr)?;

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
