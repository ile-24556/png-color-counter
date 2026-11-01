use std::fmt;
use std::io::Cursor;
use std::io::prelude::*;

use anyhow::{Result, bail, ensure};

pub struct Chunk {
    pub type_: [u8; 4],
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn new(cur: &mut Cursor<Vec<u8>>, crc_table: &[u32; 256]) -> Result<Chunk> {
        let length = u32::from_be_bytes(read_four_bytes(cur)?);
        let type_ = read_four_bytes(cur)?;
        let data = {
            let mut buf = vec![0_u8; length as usize];
            cur.read_exact(&mut buf)?;
            buf
        };
        let crc = u32::from_be_bytes(read_four_bytes(cur)?);
        ensure!(crc == calculate_crc(crc_table, &type_, &data), "wrong CRC");
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

pub struct Ihdr {
    pub width: usize,
    pub height: usize,
    pub bit_depth: u8,
    pub color_type: ColorType,
    pub compression_method: u8,
    pub filter_method: u8,
    pub interlace_method: u8,
}

impl Ihdr {
    pub fn new(data: &[u8]) -> Result<Ihdr> {
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

pub enum ColorType {
    Truecolor,
    TruecolorWithAlpha,
}

impl ColorType {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            ColorType::Truecolor => 3,
            ColorType::TruecolorWithAlpha => 4,
        }
    }
}

/// Makes a CRC table.
pub fn make_crc_table() -> [u32; 256] {
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

fn calculate_crc(crc_table: &[u32; 256], chunk_type: &[u8; 4], chunk_data: &[u8]) -> u32 {
    chunk_type
        .iter()
        .chain(chunk_data)
        .fold(0xffff_ffff, |c, &byte| {
            crc_table[(c ^ (byte as u32)) as usize & 0xff] ^ (c >> 8)
        })
        ^ 0xffff_ffff
}

/// Reads 4 bytes from cursor and returns the buffer.
fn read_four_bytes(cur: &mut Cursor<Vec<u8>>) -> Result<[u8; 4]> {
    let mut buf = [0_u8; 4];
    cur.read_exact(&mut buf)?;
    Ok(buf)
}
