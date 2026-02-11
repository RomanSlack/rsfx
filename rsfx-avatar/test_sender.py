#!/usr/bin/env python3
"""Test sender: animated gradient over Unix socket. No ML needed."""

import socket
import struct
import time
import math
import sys

SOCKET_PATH = "/tmp/rsfx-avatar.sock"
WIDTH = 120
HEIGHT = 80
FPS = 30


def send_control(sock, cmd):
    """Send RC message. cmd: 0=stop, 1=start, 2=ready"""
    sock.sendall(b"RC" + struct.pack("<B", cmd))


def send_frame(sock, width, height, rgb_data, timestamp_us):
    """Send RF message."""
    header = struct.pack("<HHQ", width, height, timestamp_us)
    sock.sendall(b"RF" + header + rgb_data)


def make_gradient_frame(width, height, t):
    """Generate an animated gradient frame. Returns RGB bytes."""
    data = bytearray(width * height * 3)
    offset = 0
    for y in range(height):
        for x in range(width):
            # Animated diagonal gradient with sine waves
            r = int((math.sin(x * 0.05 + t) * 0.5 + 0.5) * 255)
            g = int((math.sin(y * 0.05 + t * 1.3) * 0.5 + 0.5) * 255)
            b = int((math.sin((x + y) * 0.03 + t * 0.7) * 0.5 + 0.5) * 255)
            data[offset] = r
            data[offset + 1] = g
            data[offset + 2] = b
            offset += 3
    return bytes(data)


def main():
    sock_path = sys.argv[1] if len(sys.argv) > 1 else SOCKET_PATH
    width = int(sys.argv[2]) if len(sys.argv) > 2 else WIDTH
    height = int(sys.argv[3]) if len(sys.argv) > 3 else HEIGHT

    print(f"Connecting to {sock_path} ...")
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(sock_path)
    print("Connected. Sending ready...")

    send_control(sock, 2)  # Ready
    time.sleep(0.1)

    print(f"Streaming {width}x{height} gradient at {FPS} fps. Ctrl+C to stop.")
    frame_interval = 1.0 / FPS
    start = time.monotonic()
    frame_num = 0

    try:
        while True:
            t = time.monotonic() - start
            timestamp_us = int(t * 1_000_000)

            rgb = make_gradient_frame(width, height, t)
            send_frame(sock, width, height, rgb, timestamp_us)

            frame_num += 1
            # Rate limit
            next_time = start + frame_num * frame_interval
            sleep_dur = next_time - time.monotonic()
            if sleep_dur > 0:
                time.sleep(sleep_dur)
    except (KeyboardInterrupt, BrokenPipeError):
        print("\nStopping...")

    send_control(sock, 0)  # Stop
    sock.close()
    print("Done.")


if __name__ == "__main__":
    main()
