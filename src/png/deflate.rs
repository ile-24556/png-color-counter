use anyhow::{Result, ensure};
use libdeflater::Decompressor;

use crate::png::Ihdr;

pub fn decompress(compressed_data: &[u8], ihdr: &Ihdr) -> Result<Vec<u8>> {
    let mut decompressor = Decompressor::new();
    let expected_len = (1 + ihdr.color_type.bytes_per_pixel() * ihdr.width) * ihdr.height;
    let mut filtered_data = vec![0; expected_len];
    let len = decompressor.zlib_decompress(compressed_data, &mut filtered_data)?;
    ensure!(len == expected_len, "unexpected length of filtered data",);
    Ok(filtered_data)
}
