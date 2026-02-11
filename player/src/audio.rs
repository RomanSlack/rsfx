use std::io::Cursor;
use std::time::Instant;

use anyhow::Context;
use rodio::{Decoder, OutputStream, Sink};

pub struct AudioPlayer {
    _stream: OutputStream,
    sink: Sink,
    start_time: Option<Instant>,
}

impl AudioPlayer {
    pub fn new() -> anyhow::Result<Self> {
        let (stream, handle) = OutputStream::try_default().context("failed to open audio output")?;
        let sink = Sink::try_new(&handle).context("failed to create audio sink")?;
        sink.pause();

        Ok(Self {
            _stream: stream,
            sink,
            start_time: None,
        })
    }

    /// Load raw PCM s16le data and prepare for playback.
    pub fn load_pcm(&self, pcm_data: Vec<u8>, sample_rate: u32, channels: u16) -> anyhow::Result<()> {
        // Wrap PCM in a WAV header so rodio's Decoder can read it
        let wav_data = wrap_pcm_as_wav(pcm_data, sample_rate, channels);
        let cursor = Cursor::new(wav_data);
        let source = Decoder::new(cursor).context("failed to decode audio")?;
        self.sink.append(source);
        Ok(())
    }

    /// Start playback and record the start time.
    pub fn play(&mut self) {
        self.start_time = Some(Instant::now());
        self.sink.play();
    }

    /// Get elapsed playback time in seconds.
    pub fn position_secs(&self) -> f64 {
        self.start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn stop(&self) {
        self.sink.stop();
    }
}

/// Wrap raw PCM s16le data in a minimal WAV header.
fn wrap_pcm_as_wav(pcm: Vec<u8>, sample_rate: u32, channels: u16) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let file_size = 36 + data_len;

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend(pcm);
    wav
}
