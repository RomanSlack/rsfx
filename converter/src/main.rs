mod audio;
mod decode;
mod delta;
mod halfblock;
mod resize;

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use rsfx_core::encode::RsfxWriter;

use crate::decode::VideoDecoder;
use crate::delta::{compute_delta, FrameDiff};
use crate::halfblock::pixels_to_cells;
use crate::resize::FrameResizer;

#[derive(Parser)]
#[command(name = "rsfx-convert", about = "Convert MP4 video to .rsfx format")]
struct Cli {
    /// Input video file path
    input: PathBuf,

    /// Output .rsfx file path (default: input with .rsfx extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Terminal columns
    #[arg(long, default_value = "120")]
    cols: u16,

    /// Terminal rows
    #[arg(long, default_value = "40")]
    rows: u16,

    /// Frames per second (0 = auto-detect, uses 30)
    #[arg(long, default_value = "30")]
    fps: u16,

    /// Keyframe interval (frames between full keyframes)
    #[arg(long, default_value = "30")]
    keyframe_interval: u16,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let output_path = cli.output.unwrap_or_else(|| {
        let mut p = cli.input.clone();
        p.set_extension("rsfx");
        p
    });

    let input_str = cli.input.to_str().context("invalid input path")?;

    eprintln!("Decoding video: {}", cli.input.display());
    let decoder = VideoDecoder::new(input_str)?;
    eprintln!(
        "Source: {}x{} pixels",
        decoder.source_width(),
        decoder.source_height()
    );
    eprintln!(
        "Target: {}x{} cells ({}x{} pixels)",
        cli.cols,
        cli.rows,
        cli.cols,
        cli.rows * 2
    );

    let mut resizer = FrameResizer::new(cli.cols, cli.rows);

    let file = File::create(&output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    let buf_writer = BufWriter::new(file);
    let mut writer = RsfxWriter::new(buf_writer, cli.cols, cli.rows, cli.fps, cli.keyframe_interval)?;

    let mut prev_cells: Vec<rsfx_core::format::Cell> = Vec::new();
    let mut frame_num = 0u32;

    for frame in decoder {
        let resized = resizer.resize(&frame.data, frame.width, frame.height)?;
        let cells = pixels_to_cells(&resized, resizer.target_width(), resizer.target_height());

        let force_keyframe = frame_num % (cli.keyframe_interval as u32) == 0;
        let diff = compute_delta(&prev_cells, &cells, cli.cols, force_keyframe);

        match diff {
            FrameDiff::Keyframe(ref kf) => {
                writer.write_keyframe(kf)?;
            }
            FrameDiff::Delta(ref d) => {
                writer.write_delta(d)?;
            }
        }

        prev_cells = cells;
        frame_num += 1;

        if frame_num % 100 == 0 {
            eprint!("\rProcessed {frame_num} frames...");
        }
    }

    eprintln!("\rProcessed {frame_num} frames total.");

    // Extract and write audio
    eprintln!("Extracting audio...");
    match audio::extract_audio(input_str)? {
        Some(pcm) => {
            eprintln!("Audio: {} bytes PCM", pcm.len());
            writer.write_audio(&pcm, 44100, 2)?;
        }
        None => {
            eprintln!("No audio track found.");
        }
    }

    writer.finish()?;
    eprintln!("Wrote {}", output_path.display());

    Ok(())
}
