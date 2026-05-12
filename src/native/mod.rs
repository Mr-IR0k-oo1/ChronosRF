pub mod bindings;
pub mod fft;
pub mod liquid_dsp;

use crate::config::Config;
use crate::detection::peak_detector::PeakDetector;
use crate::models::{SignalPeak, SweepData};

#[allow(unused_imports)]
pub use fft::{IqSample, SpectrumFrame};
#[allow(unused_imports)]
pub use liquid_dsp::LiquidDspFft;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeRuntimeConfig {
    pub native_dsp_enabled: bool,
    pub native_fft_enabled: bool,
    pub python_ml_enabled: bool,
}

impl NativeRuntimeConfig {
    pub fn from_config(config: &Config) -> Self {
        Self {
            native_dsp_enabled: config.native_dsp_enabled,
            native_fft_enabled: config.native_fft_enabled,
            python_ml_enabled: config.python_ml_enabled,
        }
    }

    #[cfg(feature = "native-dsp")]
    pub fn uses_native_acceleration(&self) -> bool {
        self.native_dsp_enabled || self.native_fft_enabled
    }
}

#[derive(Debug)]
pub struct RustPeakAnalyzer {
    detector: PeakDetector,
}

impl RustPeakAnalyzer {
    pub fn new(threshold_db: f32) -> Self {
        Self {
            detector: PeakDetector::new(threshold_db),
        }
    }

    pub fn detect_peaks(&mut self, sweep: &SweepData) -> Vec<SignalPeak> {
        self.detector.detect(sweep)
    }
}

#[cfg(feature = "native-dsp")]
#[derive(Debug)]
pub struct NativePeakAnalyzer {
    fallback: RustPeakAnalyzer,
    backend_name: &'static str,
}

#[cfg(feature = "native-dsp")]
impl NativePeakAnalyzer {
    pub fn new(threshold_db: f32, native_runtime: NativeRuntimeConfig) -> Self {
        let backend_name = if native_runtime.native_fft_enabled {
            "native-fft-fallback"
        } else {
            "native-dsp-fallback"
        };

        Self {
            fallback: RustPeakAnalyzer::new(threshold_db),
            backend_name,
        }
    }

    pub fn detect_peaks(&mut self, sweep: &SweepData) -> Vec<SignalPeak> {
        self.fallback.detect_peaks(sweep)
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }
}

#[derive(Debug)]
pub enum PeakAnalysisBackend {
    Rust(RustPeakAnalyzer),
    #[cfg(feature = "native-dsp")]
    Native(NativePeakAnalyzer),
}

impl PeakAnalysisBackend {
    pub fn new(threshold_db: f32, _native_runtime: NativeRuntimeConfig) -> Self {
        #[cfg(feature = "native-dsp")]
        if _native_runtime.uses_native_acceleration() {
            return Self::Native(NativePeakAnalyzer::new(threshold_db, _native_runtime));
        }

        Self::Rust(RustPeakAnalyzer::new(threshold_db))
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::Rust(_) => "rust",
            #[cfg(feature = "native-dsp")]
            Self::Native(backend) => backend.backend_name(),
        }
    }

    pub fn detect_peaks(&mut self, sweep: &SweepData) -> Vec<SignalPeak> {
        match self {
            Self::Rust(backend) => backend.detect_peaks(sweep),
            #[cfg(feature = "native-dsp")]
            Self::Native(backend) => backend.detect_peaks(sweep),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeRuntimeConfig, PeakAnalysisBackend};
    use crate::models::SweepData;

    fn sweep(power_values: Vec<f32>) -> SweepData {
        SweepData {
            sequence: 1,
            captured_at_ms: 1_000,
            timestamp: "2026-05-12 12:00:00".to_string(),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_403_000_000,
            bin_width_hz: 1_000_000.0,
            sample_count: 20,
            power_values,
        }
    }

    #[test]
    fn rust_backend_is_default() {
        let mut backend = PeakAnalysisBackend::new(-35.0, NativeRuntimeConfig::default());

        assert_eq!(backend.backend_name(), "rust");
        assert_eq!(backend.detect_peaks(&sweep(vec![-40.0, -10.0])).len(), 1);
    }

    #[cfg(feature = "native-dsp")]
    #[test]
    fn native_backend_is_selected_when_enabled() {
        let native_runtime = NativeRuntimeConfig {
            native_dsp_enabled: true,
            ..NativeRuntimeConfig::default()
        };
        let backend = PeakAnalysisBackend::new(0.0, native_runtime);

        assert_eq!(backend.backend_name(), "native-dsp-fallback");
    }

    #[cfg(feature = "native-dsp")]
    #[test]
    fn native_fft_backend_is_selected_when_enabled() {
        let native_runtime = NativeRuntimeConfig {
            native_fft_enabled: true,
            ..NativeRuntimeConfig::default()
        };
        let backend = PeakAnalysisBackend::new(0.0, native_runtime);

        assert_eq!(backend.backend_name(), "native-fft-fallback");
    }
}