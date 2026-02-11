mod audio;
mod delta;
mod format;
mod halfblock;
mod protocol;
mod render;

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;

use crate::audio::StreamingSource;
use crate::delta::{compute_delta, FrameDiff};
use crate::format::Cell;
use crate::halfblock::pixels_to_cells;
use crate::protocol::{bind_listener, ControlCmd, Message, SocketReceiver};
use crate::render::{render_delta, render_keyframe};

#[derive(Parser)]
#[command(name = "rsfx-avatar", about = "Terminal avatar renderer")]
struct Cli {
    /// Unix socket path
    #[arg(short, long, default_value = "/tmp/rsfx-avatar.sock")]
    socket: PathBuf,

    /// Display width in terminal columns
    #[arg(long, default_value_t = 120)]
    cols: u16,

    /// Display height in terminal rows (half the pixel height)
    #[arg(long, default_value_t = 40)]
    rows: u16,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Bind socket
    let listener = bind_listener(&cli.socket)?;
    eprintln!(
        "rsfx-avatar: waiting for connection on {} ...",
        cli.socket.display()
    );

    // Accept one connection
    let (stream, _addr) = listener.accept().context("accepting connection")?;
    eprintln!("rsfx-avatar: connected");

    let mut receiver = SocketReceiver::new(stream);

    // Wait for Ready control message
    loop {
        match receiver.recv()? {
            Some(Message::Control(ControlCmd::Ready)) => break,
            Some(_) => continue,
            None => anyhow::bail!("connection closed before ready"),
        }
    }
    eprintln!("rsfx-avatar: received ready, entering render mode");

    // Set up audio
    let (_stream_handle, audio_handle) = setup_audio()?;

    // Enter alternate screen + raw mode
    let mut stdout = io::stdout();
    terminal::enable_raw_mode().context("enable raw mode")?;
    crossterm::execute!(
        stdout,
        terminal::EnterAlternateScreen,
        crossterm::cursor::Hide
    )
    .context("enter alt screen")?;

    // Panic hook to restore terminal
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::cursor::Show,
            terminal::LeaveAlternateScreen
        );
        orig_hook(info);
    }));

    // Spawn receiver thread
    let (tx, rx) = mpsc::channel::<Message>();
    thread::spawn(move || {
        loop {
            match receiver.recv() {
                Ok(Some(msg)) => {
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });

    // Render loop
    let result = render_loop(&cli, &rx, &audio_handle, &mut stdout);

    // Restore terminal
    let _ = terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        stdout,
        crossterm::cursor::Show,
        terminal::LeaveAlternateScreen
    );

    // Clean up socket
    let _ = std::fs::remove_file(&cli.socket);

    result
}

fn setup_audio() -> Result<(rodio::OutputStream, crate::audio::AudioHandle)> {
    let source = StreamingSource::new(16000, 1);
    let handle = source.handle();
    let (stream, stream_handle) =
        rodio::OutputStream::try_default().context("opening audio output")?;
    stream_handle
        .play_raw(source)
        .context("starting audio playback")?;
    Ok((stream, handle))
}

fn render_loop(
    cli: &Cli,
    rx: &mpsc::Receiver<Message>,
    audio_handle: &crate::audio::AudioHandle,
    stdout: &mut io::Stdout,
) -> Result<()> {
    let cols = cli.cols;
    let rows = cli.rows;

    let mut prev_cells: Vec<Cell> = Vec::new();
    let mut render_buf = Vec::with_capacity(cols as usize * rows as usize * 20);
    let mut frame_count: u64 = 0;
    let mut last_log = Instant::now();

    loop {
        // Poll keyboard (non-blocking)
        if event::poll(Duration::from_millis(1)).context("polling events")? {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read().context("reading event")?
            {
                match code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Esc => break,
                    _ => {}
                }
            }
        }

        // Process all pending messages
        loop {
            match rx.try_recv() {
                Ok(Message::Frame {
                    width,
                    height,
                    rgb_data,
                    ..
                }) => {
                    let cells = pixels_to_cells(&rgb_data, width as u32, height as u32);
                    let cell_rows = (height / 2) as u16;

                    let diff = compute_delta(&prev_cells, &cells, width, frame_count == 0);

                    match diff {
                        FrameDiff::Keyframe(ref k) => {
                            render_keyframe(k, width, cell_rows, &mut render_buf);
                        }
                        FrameDiff::Delta(ref d) => {
                            render_delta(d, &mut render_buf);
                        }
                    }

                    stdout.write_all(&render_buf)?;
                    stdout.flush()?;

                    prev_cells = cells;
                    frame_count += 1;

                    // Log latency every 30 frames
                    if frame_count % 30 == 0 {
                        let elapsed = last_log.elapsed();
                        let fps = 30.0 / elapsed.as_secs_f64();
                        // Write to alternate screen bottom or just track internally
                        let _ = fps; // avoid unused warning; can add status bar later
                        last_log = Instant::now();
                    }
                }
                Ok(Message::Audio(pcm_data)) => {
                    audio_handle.push_pcm(&pcm_data);
                }
                Ok(Message::Control(ControlCmd::Stop)) => {
                    return Ok(());
                }
                Ok(Message::Control(_)) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }

    Ok(())
}
