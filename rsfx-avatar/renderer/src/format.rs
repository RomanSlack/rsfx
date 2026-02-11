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

/// A changed cell in a delta frame: position + new cell data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeltaCell {
    pub x: u16,
    pub y: u16,
    pub cell: Cell,
}
