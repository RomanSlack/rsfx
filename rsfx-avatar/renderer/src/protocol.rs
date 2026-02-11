use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use anyhow::{bail, Context, Result};

/// Messages received over the wire protocol.
pub enum Message {
    /// RGB frame data: width, height, timestamp_us, pixel data
    Frame {
        width: u16,
        height: u16,
        timestamp_us: u64,
        rgb_data: Vec<u8>,
    },
    /// Raw PCM audio (s16le)
    Audio(Vec<u8>),
    /// Control command
    Control(ControlCmd),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCmd {
    Stop = 0,
    Start = 1,
    Ready = 2,
}

/// Binds a Unix domain socket, removing any stale socket file first.
pub fn bind_listener(path: &Path) -> Result<UnixListener> {
    if path.exists() {
        std::fs::remove_file(path).context("removing stale socket")?;
    }
    UnixListener::bind(path).context("binding unix socket")
}

/// Reads messages from a connected Unix stream.
pub struct SocketReceiver {
    stream: UnixStream,
}

impl SocketReceiver {
    pub fn new(stream: UnixStream) -> Self {
        Self { stream }
    }

    /// Read the next message from the socket. Returns None on EOF.
    pub fn recv(&mut self) -> Result<Option<Message>> {
        let mut magic = [0u8; 2];
        match self.stream.read_exact(&mut magic) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e).context("reading message magic"),
        }

        match &magic {
            b"RF" => self.read_frame(),
            b"RA" => self.read_audio(),
            b"RC" => self.read_control(),
            _ => bail!("unknown message magic: {:?}", magic),
        }
    }

    fn read_frame(&mut self) -> Result<Option<Message>> {
        let mut header = [0u8; 12]; // width:2 + height:2 + timestamp:8
        self.stream
            .read_exact(&mut header)
            .context("reading frame header")?;

        let width = u16::from_le_bytes([header[0], header[1]]);
        let height = u16::from_le_bytes([header[2], header[3]]);
        let timestamp_us = u64::from_le_bytes(header[4..12].try_into().unwrap());

        let data_len = width as usize * height as usize * 3;
        let mut rgb_data = vec![0u8; data_len];
        self.stream
            .read_exact(&mut rgb_data)
            .context("reading frame rgb data")?;

        Ok(Some(Message::Frame {
            width,
            height,
            timestamp_us,
            rgb_data,
        }))
    }

    fn read_audio(&mut self) -> Result<Option<Message>> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .context("reading audio length")?;
        let length = u32::from_le_bytes(len_buf) as usize;

        let mut pcm_data = vec![0u8; length];
        self.stream
            .read_exact(&mut pcm_data)
            .context("reading audio pcm data")?;

        Ok(Some(Message::Audio(pcm_data)))
    }

    fn read_control(&mut self) -> Result<Option<Message>> {
        let mut cmd = [0u8; 1];
        self.stream
            .read_exact(&mut cmd)
            .context("reading control command")?;

        let cmd = match cmd[0] {
            0 => ControlCmd::Stop,
            1 => ControlCmd::Start,
            2 => ControlCmd::Ready,
            other => bail!("unknown control command: {other}"),
        };

        Ok(Some(Message::Control(cmd)))
    }
}
