//! WinUI ContentDialog's two-layer ThemeShadow at Translation.Z = 128.
//! Recipe and nine-grid geometry:
//! https://github.com/microsoft/microsoft-ui-xaml/blob/f980ab26e378db4afe4264c4f00c5bf6cc11ec34/dxaml/xcp/components/graphics/inc/DropShadowRecipe.h
//! https://github.com/microsoft/microsoft-ui-xaml/blob/f980ab26e378db4afe4264c4f00c5bf6cc11ec34/dxaml/xcp/components/comptree/HWCompNodeWinRT.cpp
use super::shadow::normal_cdf;
use super::*;

pub(crate) struct DialogShadow {
    key: (u32, u32, u32, bool),
    pub pixels: Arc<Pixmap>,
}

impl DialogShadow {
    pub fn matches(&self, size: (f32, f32), scale: f32, dark: bool) -> bool {
        self.key == (size.0.to_bits(), size.1.to_bits(), scale.to_bits(), dark)
    }

    pub fn rect(&self, (x, y, w, h): Rect4) -> Rect4 {
        (x - 64., y - 32., w + 128., h + 128.)
    }

    pub fn new(size: (f32, f32), scale: f32, dark: bool) -> Option<Self> {
        let width = ((size.0 + 128.) * scale).round() as u32;
        let height = ((size.1 + 128.) * scale).round() as u32;
        let mut pixels = Pixmap::new(width, height)?;
        // WinUI stretches a 64-point caster, inset by one point. Corners
        // and shadow margins stay fixed; the 46-point middle stretches.
        let mapped = |p: f32, size: f32| {
            if p < 9. {
                p
            } else if p > size - 9. {
                p - size + 64.
            } else {
                9. + (p - 9.) * 46. / (size - 18.)
            }
        };
        let xs: Vec<_> = (0..width)
            .map(|x| mapped((x as f32 + 0.5) / scale - 64., size.0))
            .collect();
        let ys: Vec<_> = (0..height)
            .map(|y| mapped((y as f32 + 0.5) / scale - 32., size.1))
            .collect();
        let opacities = if dark { [94., 94.] } else { [38., 48.] };
        for ((blur, offset), opacity) in
            [(64. / 3. + 1., 2.), (65., 32.)].into_iter().zip(opacities)
        {
            // Composition's blur radius covers three Gaussian deviations.
            let sigma = blur / 3.;
            let mut x_coverage = Vec::new();
            let mut y_coverage = Vec::new();
            let mut strip = |left: f32, top: f32, right: f32, bottom: f32| {
                x_coverage.push(
                    xs.iter()
                        .map(|x| interval(*x, left, right, sigma))
                        .collect::<Vec<_>>(),
                );
                y_coverage.push(
                    ys.iter()
                        .map(|y| interval(*y - offset, top, bottom, sigma))
                        .collect::<Vec<_>>(),
                );
            };
            strip(1., 9., 63., 55.);
            // Integrate the rounded cap in half-point strips. The blur is
            // much wider than a strip, including at higher display scales.
            for row in 0..16 {
                let top = 1. + row as f32 * 0.5;
                let dy = 9. - (top + 0.25);
                let left = 9. - (64. - dy * dy).sqrt();
                strip(left, top, 64. - left, top + 0.5);
                strip(left, 64. - top - 0.5, 64. - left, 64. - top);
            }
            for (y, row) in pixels
                .data_mut()
                .chunks_exact_mut(width as usize * 4)
                .enumerate()
            {
                for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                    // The nine-grid's middle is hollow and covered by the dialog.
                    if (9. ..55.).contains(&xs[x]) && (9. ..55.).contains(&ys[y]) {
                        continue;
                    }
                    let coverage: f32 = x_coverage
                        .iter()
                        .zip(&y_coverage)
                        .map(|(horizontal, vertical)| horizontal[x] * vertical[y])
                        .sum();
                    let alpha = (coverage * opacity).clamp(0., 255.);
                    pixel[3] = (alpha + pixel[3] as f32 * (1. - alpha / 255.)).round() as u8;
                }
            }
        }
        Some(Self {
            key: (size.0.to_bits(), size.1.to_bits(), scale.to_bits(), dark),
            pixels: Arc::new(pixels),
        })
    }
}

fn interval(at: f32, left: f32, right: f32, sigma: f32) -> f32 {
    normal_cdf((right - at) / sigma) - normal_cdf((left - at) / sigma)
}
