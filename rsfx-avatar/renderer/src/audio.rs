use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Streaming PCM audio source for rodio.
///
/// Backed by a shared buffer of f32 samples. When empty, outputs silence (0.0)
/// to keep the audio stream alive. Push PCM data from any thread via `push_pcm()`.
pub struct StreamingSource {
    buffer: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: u32,
    channels: u16,
}

impl StreamingSource {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(sample_rate as usize))),
            sample_rate,
            channels,
        }
    }

    /// Get a handle for pushing audio data from another thread.
    pub fn handle(&self) -> AudioHandle {
        AudioHandle {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl Iterator for StreamingSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut buf = self.buffer.lock().unwrap();
        Some(buf.pop_front().unwrap_or(0.0))
    }
}

impl rodio::Source for StreamingSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Thread-safe handle for pushing PCM data into the streaming source.
pub struct AudioHandle {
    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl AudioHandle {
    /// Convert raw s16le bytes to f32 samples and append to the buffer.
    pub fn push_pcm(&self, data: &[u8]) {
        let mut buf = self.buffer.lock().unwrap();
        for chunk in data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            buf.push_back(sample as f32 / 32768.0);
        }
    }
}
