use rsfx_core::format::{Cell, DeltaCell};

/// Result of comparing two frames.
pub enum FrameDiff {
    /// Use this as a keyframe (too many changes, or no previous frame).
    Keyframe(Vec<Cell>),
    /// Delta: only the changed cells.
    Delta(Vec<DeltaCell>),
}

/// Compare current frame cells against previous, producing either a delta or promoting to keyframe.
/// `cols` is needed to compute x,y positions from the flat cell array.
pub fn compute_delta(
    prev: &[Cell],
    current: &[Cell],
    cols: u16,
    force_keyframe: bool,
) -> FrameDiff {
    if force_keyframe || prev.is_empty() {
        return FrameDiff::Keyframe(current.to_vec());
    }

    let total = current.len();
    let mut deltas = Vec::new();

    for i in 0..total {
        if current[i] != prev[i] {
            let x = (i % cols as usize) as u16;
            let y = (i / cols as usize) as u16;
            deltas.push(DeltaCell {
                x,
                y,
                cell: current[i],
            });
        }
    }

    // If >60% of cells changed, just send a keyframe
    if deltas.len() > total * 60 / 100 {
        FrameDiff::Keyframe(current.to_vec())
    } else {
        FrameDiff::Delta(deltas)
    }
}
