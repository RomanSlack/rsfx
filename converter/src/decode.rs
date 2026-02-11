use anyhow::Context;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;

pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct VideoDecoder {
    width: u32,
    height: u32,
    events: Box<dyn Iterator<Item = FfmpegEvent>>,
}

impl VideoDecoder {
    pub fn new(input_path: &str) -> anyhow::Result<Self> {
        // First probe to get dimensions
        let mut probe = FfmpegCommand::new()
            .input(input_path)
            .rawvideo()
            .spawn()
            .context("failed to spawn ffmpeg — is it installed?")?;

        let mut events = probe.iter().context("failed to iterate ffmpeg events")?;

        // Find the first output frame to get dimensions
        let mut width = 0u32;
        let mut height = 0u32;
        let mut first_frame = None;

        let mut collected: Vec<FfmpegEvent> = Vec::new();
        for event in &mut events {
            match &event {
                FfmpegEvent::OutputFrame(frame) => {
                    width = frame.width;
                    height = frame.height;
                    first_frame = Some(event);
                    break;
                }
                _ => {
                    collected.push(event);
                }
            }
        }

        if width == 0 || height == 0 {
            anyhow::bail!("could not determine video dimensions");
        }

        // Chain the first frame back with remaining events
        let rest = collected.into_iter().chain(first_frame).chain(events);

        Ok(Self {
            width,
            height,
            events: Box::new(rest),
        })
    }

    pub fn source_width(&self) -> u32 {
        self.width
    }

    pub fn source_height(&self) -> u32 {
        self.height
    }
}

impl Iterator for VideoDecoder {
    type Item = VideoFrame;

    fn next(&mut self) -> Option<Self::Item> {
        for event in &mut self.events {
            if let FfmpegEvent::OutputFrame(frame) = event {
                return Some(VideoFrame {
                    data: frame.data,
                    width: frame.width,
                    height: frame.height,
                });
            }
        }
        None
    }
}
