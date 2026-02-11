use rsfx_core::format::Cell;

/// Convert RGB pixel data into a Cell grid using the half-block trick.
/// Each cell = 2 vertical pixels: bg = top pixel, fg = bottom pixel.
///
/// Input: RGB24 data (width × height pixels, height must be even)
/// Output: Cell grid (width × height/2 cells), row-major
pub fn pixels_to_cells(rgb: &[u8], width: u32, height: u32) -> Vec<Cell> {
    let cols = width as usize;
    let rows = (height / 2) as usize;
    let stride = cols * 3;
    let mut cells = Vec::with_capacity(cols * rows);

    for row in 0..rows {
        let top_y = row * 2;
        let bot_y = top_y + 1;

        for col in 0..cols {
            let top_off = top_y * stride + col * 3;
            let bot_off = bot_y * stride + col * 3;

            cells.push(Cell {
                bg_r: rgb[top_off],
                bg_g: rgb[top_off + 1],
                bg_b: rgb[top_off + 2],
                fg_r: rgb[bot_off],
                fg_g: rgb[bot_off + 1],
                fg_b: rgb[bot_off + 2],
            });
        }
    }

    cells
}
