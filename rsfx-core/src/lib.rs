pub mod format;
pub mod compress;
pub mod encode;
pub mod decode;

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use crate::format::*;
    use crate::encode::RsfxWriter;
    use crate::decode::RsfxReader;

    #[test]
    fn roundtrip_keyframe_and_delta() {
        let cols = 4u16;
        let rows = 2u16;
        let total_cells = (cols as usize) * (rows as usize);

        // Build a keyframe grid
        let mut cells: Vec<Cell> = Vec::new();
        for i in 0..total_cells {
            let v = i as u8;
            cells.push(Cell {
                bg_r: v, bg_g: v + 10, bg_b: v + 20,
                fg_r: v + 30, fg_g: v + 40, fg_b: v + 50,
            });
        }

        // Build some deltas
        let deltas = vec![
            DeltaCell { x: 1, y: 0, cell: Cell { bg_r: 255, bg_g: 0, bg_b: 0, fg_r: 0, fg_g: 255, fg_b: 0 } },
            DeltaCell { x: 3, y: 1, cell: Cell { bg_r: 0, bg_g: 0, bg_b: 255, fg_r: 128, fg_g: 128, fg_b: 128 } },
        ];

        // Audio data
        let audio_pcm = vec![0u8; 1024];

        // Write
        let buf = Cursor::new(Vec::new());
        let mut writer = RsfxWriter::new(buf, cols, rows, 30, 30).unwrap();
        writer.write_keyframe(&cells).unwrap();
        writer.write_delta(&deltas).unwrap();
        writer.write_audio(&audio_pcm, 44100, 2).unwrap();
        let buf = writer.finish().unwrap();

        // Read back
        let mut reader = RsfxReader::new(Cursor::new(buf.into_inner())).unwrap();
        assert_eq!(reader.header.cols, cols);
        assert_eq!(reader.header.rows, rows);
        assert_eq!(reader.header.fps_num, 30);
        assert_eq!(reader.header.frame_count, 2);
        assert_eq!(reader.header.audio_sample_rate, 44100);
        assert_eq!(reader.header.audio_channels, 2);

        // Verify keyframe
        assert!(matches!(reader.frame_type(0), FrameType::Keyframe));
        let read_cells = reader.read_keyframe(0).unwrap();
        assert_eq!(read_cells, cells);

        // Verify delta
        assert!(matches!(reader.frame_type(1), FrameType::Delta));
        let read_deltas = reader.read_delta(1).unwrap();
        assert_eq!(read_deltas, deltas);

        // Verify audio
        let read_audio = reader.read_audio().unwrap();
        assert_eq!(read_audio, audio_pcm);
    }
}
