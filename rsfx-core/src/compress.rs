use lz4_flex::{compress_prepend_size, decompress_size_prepended};

pub fn compress(data: &[u8]) -> Vec<u8> {
    compress_prepend_size(data)
}

pub fn decompress(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    decompress_size_prepended(data).map_err(|e| anyhow::anyhow!("lz4 decompress failed: {e}"))
}
