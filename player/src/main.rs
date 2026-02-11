mod audio;
mod render;

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal;

use rsfx_core::decode::RsfxReader;
use rsfx_core::format::FrameType;

#[derive(Parser)]
#[command(name = "rsfx-play", about = "Play .rsfx files in the terminal")]
struct Cli {
    /// Path to .rsfx file
    input: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let file = File::open(&cli.input)
        .with_context(|| format!("failed to open {}", cli.input.display()))?;
    let mut reader = RsfxReader::new(BufReader::new(file))?;

    let cols = reader.header.cols;
    let rows = reader.header.rows;
    let fps = reader.fps();
    let frame_count = reader.header.frame_count as usize;

    // Check terminal size
    let (term_cols, term_rows) = terminal::size()?;
    if term_cols < cols || term_rows < rows {
        eprintln!(
            "Warning: terminal is {}x{} but video needs {}x{}. Resize your terminal for best results.",
            term_cols, term_rows, cols, rows
        );
    }

    // Load audio
    let mut audio_player = None;
    if reader.header.audio_length > 0 {
        let pcm = reader.read_audio()?;
        match audio::AudioPlayer::new() {
            Ok(player) => {
                player.load_pcm(pcm, reader.header.audio_sample_rate, reader.header.audio_channels)?;
                audio_player = Some(player);
            }
            Err(e) => {
                eprintln!("Warning: could not initialize audio: {e}");
            }
        }
    }

    // Set up panic hook for terminal cleanup
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = cleanup_terminal();
        original_hook(info);
    }));

    // Enter alternate screen, raw mode, hide cursor
    terminal::enable_raw_mode()?;
    let stdout = std::io::stdout();
    let mut stdout = BufWriter::with_capacity(256 * 1024, stdout.lock());
    stdout.write_all(b"\x1b[?1049h")?; // enter alternate screen
    stdout.write_all(b"\x1b[?25l")?; // hide cursor
    stdout.flush()?;

    // Show splash screen
    show_splash(&mut stdout, term_cols, term_rows)?;

    // Start audio
    if let Some(ref mut player) = audio_player {
        player.play();
    }

    let frame_duration = Duration::from_secs_f64(1.0 / fps);
    let playback_start = Instant::now();
    let mut render_buf = Vec::with_capacity(256 * 1024);
    let mut current_cells: Vec<rsfx_core::format::Cell> = Vec::new();

    let result = run_playback_loop(
        &mut reader,
        &mut stdout,
        &mut render_buf,
        &mut current_cells,
        &audio_player,
        cols,
        rows,
        frame_count,
        frame_duration,
        playback_start,
    );

    // Cleanup
    if let Some(ref player) = audio_player {
        player.stop();
    }
    stdout.write_all(b"\x1b[0m")?; // reset colors
    stdout.write_all(b"\x1b[?25h")?; // show cursor
    stdout.write_all(b"\x1b[?1049l")?; // leave alternate screen
    stdout.flush()?;
    terminal::disable_raw_mode()?;

    result
}

fn run_playback_loop<R: std::io::Read + std::io::Seek>(
    reader: &mut RsfxReader<R>,
    stdout: &mut impl Write,
    render_buf: &mut Vec<u8>,
    current_cells: &mut Vec<rsfx_core::format::Cell>,
    audio_player: &Option<audio::AudioPlayer>,
    cols: u16,
    rows: u16,
    frame_count: usize,
    frame_duration: Duration,
    playback_start: Instant,
) -> anyhow::Result<()> {
    for frame_idx in 0..frame_count {
        // Check for input (non-blocking)
        if event::poll(Duration::ZERO)? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }

        // Determine target time for this frame
        let target_time = if let Some(ref player) = audio_player {
            // Audio is master clock
            player.position_secs()
        } else {
            playback_start.elapsed().as_secs_f64()
        };

        let frame_time = frame_idx as f64 * frame_duration.as_secs_f64();

        // Skip frame if we're behind
        if frame_time + frame_duration.as_secs_f64() < target_time && frame_idx + 1 < frame_count {
            // We need to still process keyframes to keep current_cells up to date
            if matches!(reader.frame_type(frame_idx), FrameType::Keyframe) {
                *current_cells = reader.read_keyframe(frame_idx)?;
            }
            continue;
        }

        // Decode and render frame
        match reader.frame_type(frame_idx) {
            FrameType::Keyframe => {
                *current_cells = reader.read_keyframe(frame_idx)?;
                render::render_keyframe(current_cells, cols, rows, render_buf);
            }
            FrameType::Delta => {
                let deltas = reader.read_delta(frame_idx)?;
                // Apply deltas to current_cells for future reference
                for d in &deltas {
                    let idx = d.y as usize * cols as usize + d.x as usize;
                    if idx < current_cells.len() {
                        current_cells[idx] = d.cell;
                    }
                }
                render::render_delta(&deltas, render_buf);
            }
        }

        stdout.write_all(render_buf)?;
        stdout.flush()?;

        // Sleep until next frame
        let elapsed = playback_start.elapsed();
        let next_frame_time = Duration::from_secs_f64((frame_idx + 1) as f64 * frame_duration.as_secs_f64());
        if let Some(sleep_time) = next_frame_time.checked_sub(elapsed) {
            std::thread::sleep(sleep_time);
        }
    }

    Ok(())
}

fn show_splash(stdout: &mut impl Write, term_cols: u16, term_rows: u16) -> anyhow::Result<()> {
    const LOGO: &[&str] = &[
        " ######   ######  ########  ##     ##",
        " ##   ## ##       ##         ##   ## ",
        " ##   ##  ##      ##          ## ##  ",
        " ######    ####   ######       ###   ",
        " ##   ##      ##  ##          ## ##  ",
        " ##    ## ##   ## ##         ##   ## ",
        " ##     ##  ####  ##        ##     ##",
    ];

    // Clear screen with dark background
    stdout.write_all(b"\x1b[48;2;8;8;16m")?; // very dark blue-black bg
    stdout.write_all(b"\x1b[2J")?; // clear screen

    let logo_width = LOGO.iter().map(|l| l.len()).max().unwrap_or(0) as u16;
    let logo_height = LOGO.len() as u16;
    let start_row = term_rows.saturating_sub(logo_height + 4) / 2;
    let start_col = term_cols.saturating_sub(logo_width) / 2;

    // Draw logo with blue gradient
    let blues: &[(u8, u8, u8)] = &[
        (30, 90, 220),
        (50, 120, 235),
        (70, 150, 245),
        (100, 180, 255),
        (70, 150, 245),
        (50, 120, 235),
        (30, 90, 220),
    ];

    for (i, line) in LOGO.iter().enumerate() {
        let (r, g, b) = blues[i % blues.len()];
        write!(
            stdout,
            "\x1b[{};{}H\x1b[38;2;{};{};{}m{}",
            start_row + i as u16,
            start_col,
            r, g, b,
            line
        )?;
    }

    // Subtitle
    let subtitle = "terminal video engine";
    let sub_col = term_cols.saturating_sub(subtitle.len() as u16) / 2;
    write!(
        stdout,
        "\x1b[{};{}H\x1b[38;2;60;70;110m{}",
        start_row + logo_height + 2,
        sub_col,
        subtitle
    )?;

    stdout.write_all(b"\x1b[0m")?;
    stdout.flush()?;

    // Animated spinner with flashing purple gradient
    const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_row = start_row + logo_height + 4;
    let spinner_text = "loading";
    let spinner_col = term_cols.saturating_sub(spinner_text.len() as u16 + 2) / 2;

    let purples: &[(u8, u8, u8)] = &[
        (120, 40, 180),
        (150, 50, 210),
        (180, 70, 240),
        (210, 100, 255),
        (240, 140, 255),
        (210, 100, 255),
        (180, 70, 240),
        (150, 50, 210),
    ];

    let deadline = Instant::now() + Duration::from_secs(4);
    let mut tick = 0usize;
    while Instant::now() < deadline {
        let (r, g, b) = purples[tick % purples.len()];
        let spin_char = SPINNER[tick % SPINNER.len()];
        write!(
            stdout,
            "\x1b[{};{}H\x1b[48;2;8;8;16m\x1b[38;2;{};{};{}m{} {}",
            spinner_row, spinner_col, r, g, b, spin_char, spinner_text
        )?;
        stdout.flush()?;
        tick += 1;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }
    }

    // Clear for video
    stdout.write_all(b"\x1b[48;2;0;0;0m\x1b[2J")?;
    stdout.flush()?;

    Ok(())
}

fn cleanup_terminal() {
    let _ = std::io::stdout().write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l");
    let _ = std::io::stdout().flush();
    let _ = terminal::disable_raw_mode();
}
