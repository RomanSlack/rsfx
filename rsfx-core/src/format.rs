/// A single terminal cell: background color (top pixel) + foreground color (bottom pixel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Top pixel (background color)
    pub bg_r: u8,
    pub bg_g: u8,
    pub bg_b: u8,
    /// Bottom pixel (foreground color)
    pub fg_r: u8,
    pub fg_g: u8,
    pub fg_b: u8,
}

impl Cell {
    pub const SIZE: usize = 6;

    pub fn to_bytes(&self) -> [u8; 6] {
        [self.bg_r, self.bg_g, self.bg_b, self.fg_r, self.fg_g, self.fg_b]
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        Self {
            bg_r: b[0],
            bg_g: b[1],
            bg_b: b[2],
            fg_r: b[3],
            fg_g: b[4],
            fg_b: b[5],
        }
    }
}

/// A changed cell in a delta frame: position + new cell data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeltaCell {
    pub x: u16,
    pub y: u16,
    pub cell: Cell,
}

impl DeltaCell {
    pub const SIZE: usize = 10; // 2 + 2 + 6

    pub fn to_bytes(&self) -> [u8; 10] {
        let xb = self.x.to_le_bytes();
        let yb = self.y.to_le_bytes();
        let cb = self.cell.to_bytes();
        [xb[0], xb[1], yb[0], yb[1], cb[0], cb[1], cb[2], cb[3], cb[4], cb[5]]
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let x = u16::from_le_bytes([b[0], b[1]]);
        let y = u16::from_le_bytes([b[2], b[3]]);
        let cell = Cell::from_bytes(&b[4..10]);
        Self { x, y, cell }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameType {
    Keyframe = 0,
    Delta = 1,
}

impl FrameType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => FrameType::Keyframe,
            _ => FrameType::Delta,
        }
    }
}

pub const MAGIC: &[u8; 4] = b"RSFX";
pub const VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 64;

/// File header — fixed 64 bytes at the start of a .rsfx file.
#[derive(Clone, Debug)]
pub struct RsfxHeader {
    // magic: [u8; 4] = "RSFX"
    // version: u16
    pub cols: u16,
    pub rows: u16,
    pub fps_num: u16,
    pub fps_den: u16,
    pub frame_count: u32,
    pub keyframe_interval: u16,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub audio_offset: u64,
    pub audio_length: u64,
    pub index_offset: u64,
}

impl RsfxHeader {
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..6].copy_from_slice(&VERSION.to_le_bytes());
        buf[6..8].copy_from_slice(&self.cols.to_le_bytes());
        buf[8..10].copy_from_slice(&self.rows.to_le_bytes());
        buf[10..12].copy_from_slice(&self.fps_num.to_le_bytes());
        buf[12..14].copy_from_slice(&self.fps_den.to_le_bytes());
        buf[14..18].copy_from_slice(&self.frame_count.to_le_bytes());
        buf[18..20].copy_from_slice(&self.keyframe_interval.to_le_bytes());
        buf[20..24].copy_from_slice(&self.audio_sample_rate.to_le_bytes());
        buf[24..26].copy_from_slice(&self.audio_channels.to_le_bytes());
        buf[26..34].copy_from_slice(&self.audio_offset.to_le_bytes());
        buf[34..42].copy_from_slice(&self.audio_length.to_le_bytes());
        buf[42..50].copy_from_slice(&self.index_offset.to_le_bytes());
        // bytes 50..64 reserved
        buf
    }

    pub fn from_bytes(buf: &[u8; HEADER_SIZE]) -> anyhow::Result<Self> {
        if &buf[0..4] != MAGIC {
            anyhow::bail!("invalid magic: expected RSFX");
        }
        let version = u16::from_le_bytes([buf[4], buf[5]]);
        if version != VERSION {
            anyhow::bail!("unsupported version: {version}");
        }
        Ok(Self {
            cols: u16::from_le_bytes([buf[6], buf[7]]),
            rows: u16::from_le_bytes([buf[8], buf[9]]),
            fps_num: u16::from_le_bytes([buf[10], buf[11]]),
            fps_den: u16::from_le_bytes([buf[12], buf[13]]),
            frame_count: u32::from_le_bytes([buf[14], buf[15], buf[16], buf[17]]),
            keyframe_interval: u16::from_le_bytes([buf[18], buf[19]]),
            audio_sample_rate: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            audio_channels: u16::from_le_bytes([buf[24], buf[25]]),
            audio_offset: u64::from_le_bytes(buf[26..34].try_into().unwrap()),
            audio_length: u64::from_le_bytes(buf[34..42].try_into().unwrap()),
            index_offset: u64::from_le_bytes(buf[42..50].try_into().unwrap()),
        })
    }
}

/// One entry in the frame index at the end of the file.
#[derive(Clone, Copy, Debug)]
pub struct FrameIndexEntry {
    pub offset: u64,
    pub compressed_size: u32,
    pub frame_type: FrameType,
}

impl FrameIndexEntry {
    pub const SIZE: usize = 16;

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.offset.to_le_bytes());
        buf[8..12].copy_from_slice(&self.compressed_size.to_le_bytes());
        buf[12] = self.frame_type as u8;
        // bytes 13..16 reserved
        buf
    }

    pub fn from_bytes(buf: &[u8; 16]) -> Self {
        Self {
            offset: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
            compressed_size: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
            frame_type: FrameType::from_u8(buf[12]),
        }
    }
}
