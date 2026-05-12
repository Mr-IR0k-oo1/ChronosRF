use anyhow::ensure;

use crate::core::errors::Result;

use super::fft::{IqSample, SpectrumFrame, compute_spectrum};

#[derive(Clone, Debug)]
pub struct LiquidDspFft {
    fft_size: usize,
    sample_rate_hz: f64,
    center_frequency_hz: f64,
}

impl LiquidDspFft {
    pub fn new(fft_size: usize, sample_rate_hz: f64, center_frequency_hz: f64) -> Result<Self> {
        ensure!(fft_size > 0, "fft size must be greater than zero");
        ensure!(
            sample_rate_hz.is_finite() && sample_rate_hz > 0.0,
            "sample rate must be positive and finite"
        );
        ensure!(
            center_frequency_hz.is_finite(),
            "center frequency must be finite"
        );

        Ok(Self {
            fft_size,
            sample_rate_hz,
            center_frequency_hz,
        })
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn sample_rate_hz(&self) -> f64 {
        self.sample_rate_hz
    }

    pub fn center_frequency_hz(&self) -> f64 {
        self.center_frequency_hz
    }

    pub fn process_samples(&self, samples: &[IqSample]) -> Result<SpectrumFrame> {
        compute_spectrum(
            samples,
            self.fft_size,
            self.sample_rate_hz,
            self.center_frequency_hz,
        )
    }

    pub fn native_acceleration_ready() -> bool {
        #[cfg(has_liquid_dsp)]
        {
            let _ = crate::native::bindings::LIQUID_FFT_FORWARD;
            true
        }

        #[cfg(not(has_liquid_dsp))]
        {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LiquidDspFft;
    use crate::native::fft::IqSample;

    #[test]
    fn wrapper_processes_iq_samples_safely() {
        let fft = LiquidDspFft::new(4, 1_000.0, 2_400_000_000.0)
            .expect("wrapper should construct");
        let samples = vec![IqSample::new(1.0, 0.0); 4];

        let frame = fft.process_samples(&samples).expect("wrapper should process samples");

        assert_eq!(frame.fft_size, 4);
        assert_eq!(frame.bin_frequencies_hz[2], 2_400_000_000.0);
        assert_eq!(frame.power_dbfs.len(), 4);
        assert_eq!(fft.fft_size(), 4);
        assert_eq!(fft.sample_rate_hz(), 1_000.0);
        assert_eq!(fft.center_frequency_hz(), 2_400_000_000.0);
        assert_eq!(LiquidDspFft::native_acceleration_ready(), cfg!(has_liquid_dsp));
    }
}