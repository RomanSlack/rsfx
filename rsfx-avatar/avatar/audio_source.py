"""Audio source providers for the avatar pipeline."""

import numpy as np


class FileAudioSource:
    """Load an audio file and yield chunks of (float32, s16le_pcm) data."""

    def __init__(self, audio_path: str, sr: int = 16000, chunk_duration: float = 1.0):
        import librosa

        self.sr = sr
        self.chunk_duration = chunk_duration
        self.chunk_samples = int(sr * chunk_duration)

        # Load and resample to target rate
        self.audio, _ = librosa.load(audio_path, sr=sr, mono=True)
        self.position = 0

    def __iter__(self):
        return self

    def __next__(self):
        """Returns (float32_chunk, s16le_pcm_bytes) or raises StopIteration."""
        if self.position >= len(self.audio):
            raise StopIteration

        end = min(self.position + self.chunk_samples, len(self.audio))
        chunk_f32 = self.audio[self.position : end]
        self.position = end

        # Convert float32 [-1, 1] to s16le bytes
        pcm_s16 = np.clip(chunk_f32 * 32767, -32768, 32767).astype(np.int16)
        pcm_bytes = pcm_s16.tobytes()

        return chunk_f32, pcm_bytes

    @property
    def total_duration(self) -> float:
        return len(self.audio) / self.sr
