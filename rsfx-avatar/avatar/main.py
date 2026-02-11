#!/usr/bin/env python3
"""rsfx-avatar: Drive a talking head avatar over Unix socket to the terminal renderer."""

import argparse
import os
import time

import numpy as np

from frame_sender import FrameSender
from musetalk_worker import MuseTalkWorker


def main():
    parser = argparse.ArgumentParser(description="MuseTalk avatar pipeline")
    parser.add_argument("--reference", required=True, help="Path to reference face image")
    parser.add_argument("--audio", required=True, help="Path to audio file")
    parser.add_argument("--socket", default="/tmp/rsfx-avatar.sock", help="Unix socket path")
    parser.add_argument("--cols", type=int, default=120, help="Target width in pixels")
    parser.add_argument("--rows", type=int, default=80, help="Target height in pixels (must be even)")
    parser.add_argument("--fps", type=int, default=25, help="Target frame rate")
    parser.add_argument("--musetalk-dir", default=None, help="MuseTalk repo directory")
    parser.add_argument("--device", default=None, help="Device (cuda/cpu)")
    parser.add_argument("--no-float16", action="store_true", help="Use float32 instead of float16")
    parser.add_argument("--batch-size", type=int, default=8, help="Inference batch size")
    args = parser.parse_args()

    # Auto-detect MuseTalk directory
    musetalk_dir = args.musetalk_dir
    if musetalk_dir is None:
        # Try sibling directory
        script_dir = os.path.dirname(os.path.abspath(__file__))
        candidate = os.path.join(os.path.dirname(script_dir), "MuseTalk")
        if os.path.isdir(candidate):
            musetalk_dir = candidate
        else:
            parser.error("Cannot find MuseTalk directory. Use --musetalk-dir.")

    # Load MuseTalk
    print("Loading MuseTalk models...")
    worker = MuseTalkWorker(
        musetalk_dir=musetalk_dir,
        device=args.device,
        use_float16=not args.no_float16,
    )
    worker.load_models()

    # Prepare reference image
    print(f"Preparing reference image: {args.reference}")
    worker.prepare_reference(args.reference, target_width=args.cols, target_height=args.rows)

    # Process full audio file → whisper features
    print(f"Processing audio: {args.audio}")
    whisper_chunks = worker.process_audio(args.audio, fps=args.fps)
    num_frames = len(whisper_chunks)
    print(f"Generated {num_frames} whisper chunks ({num_frames / args.fps:.1f}s at {args.fps}fps)")

    # Load PCM audio for playback
    import librosa
    audio_f32, _ = librosa.load(args.audio, sr=16000, mono=True)
    pcm_s16 = np.clip(audio_f32 * 32767, -32768, 32767).astype(np.int16)
    pcm_bytes = pcm_s16.tobytes()

    # Connect to renderer
    print(f"Connecting to renderer at {args.socket}...")
    sender = FrameSender(args.socket)
    sender.connect()
    sender.send_control(2)  # Ready
    time.sleep(0.1)

    # Send audio upfront (buffers in rodio on the renderer side)
    print("Sending audio to renderer...")
    sender.send_audio(pcm_bytes)

    # Generate and stream frames
    print("Streaming frames...")
    frame_interval = 1.0 / args.fps
    start_time = time.monotonic()
    frame_num = 0

    try:
        for frame in worker.generate_frames(whisper_chunks, batch_size=args.batch_size):
            timestamp_us = int((time.monotonic() - start_time) * 1_000_000)
            sender.send_frame(frame, timestamp_us)
            frame_num += 1

            # Rate limit to target fps
            target_time = start_time + frame_num * frame_interval
            sleep_dur = target_time - time.monotonic()
            if sleep_dur > 0:
                time.sleep(sleep_dur)

    except KeyboardInterrupt:
        print("\nInterrupted.")
    except BrokenPipeError:
        print("Renderer disconnected.")

    print(f"Done. Sent {frame_num} frames.")
    sender.close()


if __name__ == "__main__":
    main()
