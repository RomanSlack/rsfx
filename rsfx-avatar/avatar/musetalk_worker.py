"""MuseTalk model wrapper for generating talking head frames."""

import os
import sys
import tempfile

import cv2
import numpy as np
import torch


class MuseTalkWorker:
    """Loads MuseTalk models and generates face animation frames from audio chunks."""

    def __init__(self, musetalk_dir: str, device: str = None, use_float16: bool = True):
        self.musetalk_dir = musetalk_dir
        self.device = torch.device(device or ("cuda" if torch.cuda.is_available() else "cpu"))
        self.use_float16 = use_float16
        self.weight_dtype = torch.float16 if use_float16 else torch.float32

        self.target_size = None
        self.reference_image = None
        self.face_box = None
        self.input_latent = None
        self._loaded = False

        # Ensure MuseTalk is importable
        if musetalk_dir not in sys.path:
            sys.path.insert(0, musetalk_dir)

    def load_models(self):
        """Load MuseTalk VAE, UNet, PE, Whisper, and face parser."""
        from musetalk.utils.utils import load_all_model
        from musetalk.utils.audio_processor import AudioProcessor
        from musetalk.utils.face_parsing import FaceParsing
        from transformers import WhisperModel

        models_dir = os.path.join(self.musetalk_dir, "models")

        # Load VAE + UNet + PositionalEncoding
        self.vae, self.unet, self.pe = load_all_model(
            unet_model_path=os.path.join(models_dir, "musetalkV15", "unet.pth"),
            vae_type=os.path.join(models_dir, "sd-vae"),
            unet_config=os.path.join(models_dir, "musetalkV15", "musetalk.json"),
            device=self.device,
        )

        # Set precision and move to device
        if self.use_float16:
            self.pe = self.pe.half()
            self.vae.vae = self.vae.vae.half()
            self.unet.model = self.unet.model.half()

        self.pe = self.pe.to(self.device)
        self.vae.vae = self.vae.vae.to(self.device)
        self.unet.model = self.unet.model.to(self.device)

        self.timesteps = torch.tensor([0], device=self.device)

        # Load Whisper
        whisper_dir = os.path.join(models_dir, "whisper")
        self.audio_processor = AudioProcessor(feature_extractor_path=whisper_dir)
        self.whisper = WhisperModel.from_pretrained(whisper_dir)
        self.whisper = self.whisper.to(device=self.device, dtype=self.weight_dtype).eval()
        self.whisper.requires_grad_(False)

        # Face parser for blending — MuseTalk uses relative paths, so chdir temporarily
        prev_cwd = os.getcwd()
        os.chdir(self.musetalk_dir)
        self.fp = FaceParsing(left_cheek_width=90, right_cheek_width=90)
        os.chdir(prev_cwd)

        self._loaded = True
        print(f"MuseTalk loaded on {self.device} ({'fp16' if self.use_float16 else 'fp32'})")

    def prepare_reference(self, image_path: str, target_width: int = 120, target_height: int = 80):
        """Pre-process the reference face image: detect face, encode with VAE."""
        from musetalk.utils.preprocessing import get_landmark_and_bbox, coord_placeholder

        self.target_size = (target_width, target_height)

        # Detect face and get bounding box
        coord_list, frame_list = get_landmark_and_bbox([image_path], 0)
        if not coord_list or coord_list[0] == coord_placeholder:
            raise ValueError(f"No face detected in: {image_path}")

        self.face_box = coord_list[0]
        self.reference_image = frame_list[0]  # BGR numpy array

        # Crop face and get VAE latents
        x1, y1, x2, y2 = self.face_box
        crop = self.reference_image[y1:y2, x1:x2]
        crop = cv2.resize(crop, (256, 256), interpolation=cv2.INTER_LANCZOS4)
        self.input_latent = self.vae.get_latents_for_unet(crop)  # [1, 8, 32, 32]

        print(f"Reference prepared: face at ({x1},{y1})-({x2},{y2})")

    def process_audio(self, audio_path: str, fps: int = 25):
        """Extract whisper features from a full audio file.

        Returns whisper_chunks tensor [num_frames, 50, 384].
        """
        whisper_input_features, librosa_length = self.audio_processor.get_audio_feature(audio_path)
        whisper_chunks = self.audio_processor.get_whisper_chunk(
            whisper_input_features,
            self.device,
            self.weight_dtype,
            self.whisper,
            librosa_length,
            fps=fps,
            audio_padding_length_left=2,
            audio_padding_length_right=2,
        )
        return whisper_chunks

    @torch.no_grad()
    def generate_frames(self, whisper_chunks, batch_size: int = 8):
        """Generate face animation frames from whisper feature chunks.

        Args:
            whisper_chunks: tensor [num_frames, 50, 384]
            batch_size: inference batch size

        Yields:
            RGB numpy arrays (target_height, target_width, 3) one at a time
        """
        from musetalk.utils.blending import get_image

        num_frames = len(whisper_chunks)

        for batch_start in range(0, num_frames, batch_size):
            batch_end = min(batch_start + batch_size, num_frames)
            whisper_batch = whisper_chunks[batch_start:batch_end].to(self.device)
            cur_batch_size = batch_end - batch_start

            # Positional encoding
            audio_feature_batch = self.pe(whisper_batch)

            # Repeat reference latent for batch
            latent_batch = self.input_latent.repeat(cur_batch_size, 1, 1, 1)
            latent_batch = latent_batch.to(dtype=self.weight_dtype)

            # UNet inference
            pred_latents = self.unet.model(
                latent_batch,
                self.timesteps,
                encoder_hidden_states=audio_feature_batch,
            ).sample

            # VAE decode → BGR numpy arrays [batch, 256, 256, 3]
            recon_frames = self.vae.decode_latents(pred_latents)

            # Composite each frame onto reference
            for recon in recon_frames:
                x1, y1, x2, y2 = self.face_box
                recon_resized = cv2.resize(recon.astype(np.uint8), (x2 - x1, y2 - y1))

                composite = get_image(
                    self.reference_image.copy(),
                    recon_resized,
                    [x1, y1, x2, y2],
                    mode="jaw",
                    fp=self.fp,
                )

                # Resize to terminal target size
                composite = cv2.resize(composite, self.target_size)
                # BGR → RGB for the wire protocol
                composite = cv2.cvtColor(composite, cv2.COLOR_BGR2RGB)
                yield composite
