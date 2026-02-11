use std::io::{Read, Seek, SeekFrom};

use crate::compress;
use crate::format::*;

/// Reads .rsfx files.
pub struct RsfxReader<R: Read + Seek> {
    reader: R,
    pub header: RsfxHeader,
    pub index: Vec<FrameIndexEntry>,
}

impl<R: Read + Seek> RsfxReader<R> {
    /// Open and parse header + index.
    pub fn new(mut reader: R) -> anyhow::Result<Self> {
        // Read header
        let mut header_buf = [0u8; HEADER_SIZE];
        reader.read_exact(&mut header_buf)?;
        let header = RsfxHeader::from_bytes(&header_buf)?;

        // Read frame index
        reader.seek(SeekFrom::Start(header.index_offset))?;
        let mut index = Vec::with_capacity(header.frame_count as usize);
        for _ in 0..header.frame_count {
            let mut entry_buf = [0u8; FrameIndexEntry::SIZE];
            reader.read_exact(&mut entry_buf)?;
            index.push(FrameIndexEntry::from_bytes(&entry_buf));
        }

        Ok(Self { reader, header, index })
    }

    /// Read and decompress a single frame by index. Returns raw bytes.
    pub fn read_frame_raw(&mut self, frame_idx: usize) -> anyhow::Result<Vec<u8>> {
        let entry = self.index[frame_idx];
        self.reader.seek(SeekFrom::Start(entry.offset))?;
        let mut compressed = vec![0u8; entry.compressed_size as usize];
        self.reader.read_exact(&mut compressed)?;
        compress::decompress(&compressed)
    }

    /// Read a keyframe as a Cell grid.
    pub fn read_keyframe(&mut self, frame_idx: usize) -> anyhow::Result<Vec<Cell>> {
        let raw = self.read_frame_raw(frame_idx)?;
        let cell_count = raw.len() / Cell::SIZE;
        let mut cells = Vec::with_capacity(cell_count);
        for i in 0..cell_count {
            cells.push(Cell::from_bytes(&raw[i * Cell::SIZE..(i + 1) * Cell::SIZE]));
        }
        Ok(cells)
    }

    /// Read a delta frame as a list of DeltaCells.
    pub fn read_delta(&mut self, frame_idx: usize) -> anyhow::Result<Vec<DeltaCell>> {
        let raw = self.read_frame_raw(frame_idx)?;
        let count = raw.len() / DeltaCell::SIZE;
        let mut deltas = Vec::with_capacity(count);
        for i in 0..count {
            deltas.push(DeltaCell::from_bytes(&raw[i * DeltaCell::SIZE..(i + 1) * DeltaCell::SIZE]));
        }
        Ok(deltas)
    }

    /// Read audio PCM data.
    pub fn read_audio(&mut self) -> anyhow::Result<Vec<u8>> {
        if self.header.audio_length == 0 {
            return Ok(Vec::new());
        }
        self.reader.seek(SeekFrom::Start(self.header.audio_offset))?;
        let mut buf = vec![0u8; self.header.audio_length as usize];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn frame_type(&self, frame_idx: usize) -> FrameType {
        self.index[frame_idx].frame_type
    }

    pub fn fps(&self) -> f64 {
        self.header.fps_num as f64 / self.header.fps_den as f64
    }
}
