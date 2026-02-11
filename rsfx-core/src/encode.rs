use std::io::{Seek, SeekFrom, Write};

use crate::compress;
use crate::format::*;

/// Writes .rsfx files incrementally.
pub struct RsfxWriter<W: Write + Seek> {
    writer: W,
    header: RsfxHeader,
    index: Vec<FrameIndexEntry>,
    frame_count: u32,
}

impl<W: Write + Seek> RsfxWriter<W> {
    /// Create a new writer. Writes a placeholder header immediately.
    pub fn new(mut writer: W, cols: u16, rows: u16, fps: u16, keyframe_interval: u16) -> anyhow::Result<Self> {
        let header = RsfxHeader {
            cols,
            rows,
            fps_num: fps,
            fps_den: 1,
            frame_count: 0,
            keyframe_interval,
            audio_sample_rate: 0,
            audio_channels: 0,
            audio_offset: 0,
            audio_length: 0,
            index_offset: 0,
        };
        // Write placeholder header
        writer.write_all(&header.to_bytes())?;
        Ok(Self {
            writer,
            header,
            index: Vec::new(),
            frame_count: 0,
        })
    }

    /// Write a keyframe (full cell grid, row-major).
    pub fn write_keyframe(&mut self, cells: &[Cell]) -> anyhow::Result<()> {
        let mut raw = Vec::with_capacity(cells.len() * Cell::SIZE);
        for c in cells {
            raw.extend_from_slice(&c.to_bytes());
        }
        let compressed = compress::compress(&raw);
        let offset = self.writer.stream_position()?;
        self.writer.write_all(&compressed)?;

        self.index.push(FrameIndexEntry {
            offset,
            compressed_size: compressed.len() as u32,
            frame_type: FrameType::Keyframe,
        });
        self.frame_count += 1;
        Ok(())
    }

    /// Write a delta frame (list of changed cells).
    pub fn write_delta(&mut self, deltas: &[DeltaCell]) -> anyhow::Result<()> {
        let mut raw = Vec::with_capacity(deltas.len() * DeltaCell::SIZE);
        for d in deltas {
            raw.extend_from_slice(&d.to_bytes());
        }
        let compressed = compress::compress(&raw);
        let offset = self.writer.stream_position()?;
        self.writer.write_all(&compressed)?;

        self.index.push(FrameIndexEntry {
            offset,
            compressed_size: compressed.len() as u32,
            frame_type: FrameType::Delta,
        });
        self.frame_count += 1;
        Ok(())
    }

    /// Write raw PCM audio data. Call after all frames.
    pub fn write_audio(&mut self, pcm_data: &[u8], sample_rate: u32, channels: u16) -> anyhow::Result<()> {
        let offset = self.writer.stream_position()?;
        self.writer.write_all(pcm_data)?;
        self.header.audio_offset = offset;
        self.header.audio_length = pcm_data.len() as u64;
        self.header.audio_sample_rate = sample_rate;
        self.header.audio_channels = channels;
        Ok(())
    }

    /// Finalize: write frame index, update header, flush.
    pub fn finish(mut self) -> anyhow::Result<W> {
        // Write frame index
        let index_offset = self.writer.stream_position()?;
        for entry in &self.index {
            self.writer.write_all(&entry.to_bytes())?;
        }

        // Update header
        self.header.frame_count = self.frame_count;
        self.header.index_offset = index_offset;

        // Seek back and rewrite header
        self.writer.seek(SeekFrom::Start(0))?;
        self.writer.write_all(&self.header.to_bytes())?;

        // Seek to end
        self.writer.seek(SeekFrom::End(0))?;
        self.writer.flush()?;

        Ok(self.writer)
    }
}
