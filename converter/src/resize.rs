use anyhow::Context;
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

pub struct FrameResizer {
    target_width: u32,
    target_height: u32,
    resizer: Resizer,
    options: ResizeOptions,
}

impl FrameResizer {
    pub fn new(target_cols: u16, target_rows: u16) -> Self {
        let target_width = target_cols as u32;
        // Each row = 2 pixels tall (half-block trick)
        let target_height = (target_rows as u32) * 2;

        Self {
            target_width,
            target_height,
            resizer: Resizer::new(),
            options: ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)),
        }
    }

    /// Resize an RGB24 frame to target dimensions. Returns RGB24 data.
    pub fn resize(&mut self, src_data: &[u8], src_width: u32, src_height: u32) -> anyhow::Result<Vec<u8>> {
        if src_width == self.target_width && src_height == self.target_height {
            return Ok(src_data.to_vec());
        }

        let src_image = Image::from_vec_u8(src_width, src_height, src_data.to_vec(), PixelType::U8x3)
            .context("failed to create source image")?;

        let mut dst_image = Image::new(self.target_width, self.target_height, PixelType::U8x3);

        self.resizer
            .resize(&src_image, &mut dst_image, &self.options)
            .context("resize failed")?;

        Ok(dst_image.into_vec())
    }

    pub fn target_width(&self) -> u32 {
        self.target_width
    }

    pub fn target_height(&self) -> u32 {
        self.target_height
    }
}
