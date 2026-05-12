use std::f32::consts::TAU;

use anyhow::ensure;

use crate::core::errors::Result;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IqSample {
    pub i: f32,
    pub q: f32,
}

impl IqSample {
    pub fn new(i: f32, q: f32) -> Self {
        Self { i, q }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumFrame {
    pub fft_size: usize,
    pub sample_rate_hz: f64,
    pub center_frequency_hz: f64,
    pub bin_spacing_hz: f64,
    pub bin_frequencies_hz: Vec<f64>,
    pub power_dbfs: Vec<f32>,
}

pub fn validate_fft_size(fft_size: usize) -> Result<()> {
    ensure!(fft_size > 0, "fft size must be greater than zero");
    Ok(())
}

pub fn compute_spectrum(
    samples: &[IqSample],
    fft_size: usize,
    sample_rate_hz: f64,
    center_frequency_hz: f64,
) -> Result<SpectrumFrame> {
    validate_fft_size(fft_size)?;
    ensure!(
        samples.len() >= fft_size,
        "expected at least {fft_size} IQ samples, found {}",
        samples.len()
    );
    ensure!(
        sample_rate_hz.is_finite() && sample_rate_hz > 0.0,
        "sample rate must be positive and finite"
    );
    ensure!(
        center_frequency_hz.is_finite(),
        "center frequency must be finite"
    );

    let bin_spacing_hz = sample_rate_hz / fft_size as f64;
    let fft_size_f32 = fft_size as f32;
    let half = fft_size / 2;
    let mut bin_frequencies_hz = Vec::with_capacity(fft_size);
    let mut power_dbfs = Vec::with_capacity(fft_size);

    for shifted_bin in 0..fft_size {
        let bin = (shifted_bin + half) % fft_size;
        let mut real = 0.0f32;
        let mut imag = 0.0f32;

        for (sample_index, sample) in samples.iter().take(fft_size).enumerate() {
            let angle = -TAU * bin as f32 * sample_index as f32 / fft_size_f32;
            let cos = angle.cos();
            let sin = angle.sin();

            real += sample.i * cos - sample.q * sin;
            imag += sample.i * sin + sample.q * cos;
        }

        let power = (real * real + imag * imag) / fft_size_f32.max(1.0);
        power_dbfs.push(10.0 * power.max(1.0e-12).log10());

        let offset_bins = shifted_bin as f64 - half as f64;
        bin_frequencies_hz.push(center_frequency_hz + offset_bins * bin_spacing_hz);
    }

    Ok(SpectrumFrame {
        fft_size,
        sample_rate_hz,
        center_frequency_hz,
        bin_spacing_hz,
        bin_frequencies_hz,
        power_dbfs,
    })
}

#[cfg(test)]
mod tests {
    use super::{IqSample, compute_spectrum};

    #[test]
    fn compute_spectrum_places_dc_bin_in_the_center_of_shifted_output() {
        let samples = vec![IqSample::new(1.0, 0.0); 4];

        let frame = compute_spectrum(&samples, 4, 1_000.0, 2_400_000_000.0)
            .expect("spectrum should compute");

        assert_eq!(frame.fft_size, 4);
        assert_eq!(frame.bin_frequencies_hz[2], 2_400_000_000.0);

        let peak_index = frame
            .power_dbfs
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.partial_cmp(right.1).expect("values should be ordered"))
            .map(|(index, _)| index)
            .expect("spectrum should contain bins");

        assert_eq!(peak_index, 2);
    }

    #[test]
    fn compute_spectrum_rejects_too_few_samples() {
        let samples = vec![IqSample::new(1.0, 0.0); 2];

        let error = compute_spectrum(&samples, 4, 1_000.0, 2_400_000_000.0)
            .expect_err("input should be rejected");

        assert!(error.to_string().contains("expected at least 4 IQ samples"));
    }
}