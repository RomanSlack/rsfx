"""Unix socket client for sending frames and audio to the Rust renderer."""

import socket
import struct

import numpy as np


class FrameSender:
    def __init__(self, socket_path: str):
        self.socket_path = socket_path
        self.sock = None

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(self.socket_path)

    def send_frame(self, rgb: np.ndarray, timestamp_us: int):
        """Send an RGB frame. rgb shape: (height, width, 3), dtype uint8."""
        height, width = rgb.shape[:2]
        header = struct.pack("<HHQ", width, height, timestamp_us)
        self.sock.sendall(b"RF" + header + rgb.tobytes())

    def send_audio(self, pcm_bytes: bytes):
        """Send raw PCM audio data (s16le)."""
        header = struct.pack("<I", len(pcm_bytes))
        self.sock.sendall(b"RA" + header + pcm_bytes)

    def send_control(self, cmd: int):
        """Send control command. 0=stop, 1=start, 2=ready."""
        self.sock.sendall(b"RC" + struct.pack("<B", cmd))

    def close(self):
        if self.sock:
            try:
                self.send_control(0)  # stop
            except Exception:
                pass
            self.sock.close()
            self.sock = None
