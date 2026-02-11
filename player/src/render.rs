use rsfx_core::format::{Cell, DeltaCell};

const HALF_BLOCK: &str = "▄";

/// Render a full keyframe to an ANSI byte buffer.
/// Writes every cell, row by row, with color optimization (skip escape if same as previous).
pub fn render_keyframe(cells: &[Cell], cols: u16, rows: u16, buf: &mut Vec<u8>) {
    buf.clear();

    // Move cursor to top-left
    buf.extend_from_slice(b"\x1b[H");

    let mut prev_bg = (255u8, 255u8, 255u8);
    let mut prev_fg = (255u8, 255u8, 255u8);
    let mut first = true;

    for row in 0..rows as usize {
        if row > 0 {
            buf.extend_from_slice(b"\r\n");
        }
        for col in 0..cols as usize {
            let cell = &cells[row * cols as usize + col];
            let bg = (cell.bg_r, cell.bg_g, cell.bg_b);
            let fg = (cell.fg_r, cell.fg_g, cell.fg_b);

            if first || bg != prev_bg {
                write_bg(buf, bg.0, bg.1, bg.2);
                prev_bg = bg;
            }
            if first || fg != prev_fg {
                write_fg(buf, fg.0, fg.1, fg.2);
                prev_fg = fg;
            }
            first = false;

            buf.extend_from_slice(HALF_BLOCK.as_bytes());
        }
    }

    // Reset colors
    buf.extend_from_slice(b"\x1b[0m");
}

/// Render a delta frame: only update changed cells.
pub fn render_delta(deltas: &[DeltaCell], buf: &mut Vec<u8>) {
    buf.clear();

    for d in deltas {
        // Move cursor to position (1-indexed)
        write_cursor_pos(buf, d.y + 1, d.x + 1);
        write_bg(buf, d.cell.bg_r, d.cell.bg_g, d.cell.bg_b);
        write_fg(buf, d.cell.fg_r, d.cell.fg_g, d.cell.fg_b);
        buf.extend_from_slice(HALF_BLOCK.as_bytes());
    }
}

fn write_bg(buf: &mut Vec<u8>, r: u8, g: u8, b: u8) {
    buf.extend_from_slice(b"\x1b[48;2;");
    write_u8(buf, r);
    buf.push(b';');
    write_u8(buf, g);
    buf.push(b';');
    write_u8(buf, b);
    buf.push(b'm');
}

fn write_fg(buf: &mut Vec<u8>, r: u8, g: u8, b: u8) {
    buf.extend_from_slice(b"\x1b[38;2;");
    write_u8(buf, r);
    buf.push(b';');
    write_u8(buf, g);
    buf.push(b';');
    write_u8(buf, b);
    buf.push(b'm');
}

fn write_cursor_pos(buf: &mut Vec<u8>, row: u16, col: u16) {
    buf.extend_from_slice(b"\x1b[");
    write_u16(buf, row);
    buf.push(b';');
    write_u16(buf, col);
    buf.push(b'H');
}

/// Fast integer-to-ASCII for u8 values (0-255), no allocation.
fn write_u8(buf: &mut Vec<u8>, v: u8) {
    if v >= 100 {
        buf.push(b'0' + v / 100);
        buf.push(b'0' + (v / 10) % 10);
        buf.push(b'0' + v % 10);
    } else if v >= 10 {
        buf.push(b'0' + v / 10);
        buf.push(b'0' + v % 10);
    } else {
        buf.push(b'0' + v);
    }
}

fn write_u16(buf: &mut Vec<u8>, v: u16) {
    if v >= 10000 {
        buf.push(b'0' + (v / 10000) as u8);
        buf.push(b'0' + ((v / 1000) % 10) as u8);
        buf.push(b'0' + ((v / 100) % 10) as u8);
        buf.push(b'0' + ((v / 10) % 10) as u8);
        buf.push(b'0' + (v % 10) as u8);
    } else if v >= 1000 {
        buf.push(b'0' + (v / 1000) as u8);
        buf.push(b'0' + ((v / 100) % 10) as u8);
        buf.push(b'0' + ((v / 10) % 10) as u8);
        buf.push(b'0' + (v % 10) as u8);
    } else if v >= 100 {
        buf.push(b'0' + (v / 100) as u8);
        buf.push(b'0' + ((v / 10) % 10) as u8);
        buf.push(b'0' + (v % 10) as u8);
    } else if v >= 10 {
        buf.push(b'0' + (v / 10) as u8);
        buf.push(b'0' + (v % 10) as u8);
    } else {
        buf.push(b'0' + v as u8);
    }
}
