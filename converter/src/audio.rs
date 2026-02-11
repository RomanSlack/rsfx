use std::process::Command;

/// Extract audio from a video file as raw PCM s16le, 44100Hz, stereo.
/// Returns None if the video has no audio track.
pub fn extract_audio(input_path: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let output = Command::new("ffmpeg")
        .args([
            "-i", input_path,
            "-vn",
            "-acodec", "pcm_s16le",
            "-ar", "44100",
            "-ac", "2",
            "-f", "s16le",
            "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;

    if output.stdout.is_empty() {
        // No audio track or ffmpeg failed to extract audio
        return Ok(None);
    }

    Ok(Some(output.stdout))
}
