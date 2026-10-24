//! Loudness of a sound channel after EBU R 128 / ITU-R BS.1770-4 (user 2026-10-24: A1 and A2
//! normalised to the project's EBU targets). A meter takes the samples of one mono channel in
//! order and keeps, for every 100 ms of it, the mean square of the K-weighted signal and the
//! true peak (4x oversampled). From those segments any stretch of any clip, at any gain, gives
//! its integrated loudness (400 ms blocks with 75 % overlap, absolute gate -70 LUFS, relative
//! gate -10 LU) and its true peak, without reading the sound again. It reads no file, decodes
//! nothing and writes nothing.

use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.loudness";

/// The length of one segment in milliseconds (the hop of the 400 ms blocks).
pub const SEGMENT_MS: u32 = 100;

/// What a meter kept of one channel: per 100 ms segment the mean square of the K-weighted
/// signal and the true peak (linear, 1.0 = 0 dBTP). The last segment may be shorter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelLoudness {
    pub segment_ms: u32,
    pub energies: Vec<f32>,
    pub true_peaks: Vec<f32>,
}

impl ChannelLoudness {
    pub fn validate(&self) -> Result<(), String> {
        if self.segment_ms != SEGMENT_MS || self.energies.len() != self.true_peaks.len() {
            return Err("loudness segments do not fit".into());
        }
        if self.energies.iter().chain(&self.true_peaks).any(|value| !value.is_finite() || *value < 0.0) {
            return Err("loudness value out of range".into());
        }
        Ok(())
    }
}

/// One mono channel measured as it is played (BS.1770: channel weight 1).
pub struct Meter {
    shelf: Biquad,
    high_pass: Biquad,
    segment_samples: u64,
    in_segment: u64,
    sum: f64,
    peak: f32,
    oversample: Option<TruePeak>,
    kept: ChannelLoudness,
}

impl Meter {
    /// A meter for a channel at `sample_rate_hz` (8 kHz to 768 kHz).
    pub fn new(sample_rate_hz: u32) -> Result<Self, String> {
        if !(8_000..=768_000).contains(&sample_rate_hz) {
            return Err(format!("loudness: sample rate {sample_rate_hz} Hz is not measured"));
        }
        let rate = f64::from(sample_rate_hz);
        Ok(Self {
            shelf: Biquad::shelf(rate),
            high_pass: Biquad::high_pass(rate),
            segment_samples: (u64::from(sample_rate_hz) * u64::from(SEGMENT_MS)).div_ceil(1000),
            in_segment: 0,
            sum: 0.0,
            peak: 0.0,
            // BS.1770-4 Annex 2: 4x oversampling below 96 kHz; at 96 kHz and above the
            // samples are close enough (2x would do, the sample peak is kept).
            oversample: (sample_rate_hz < 96_000).then(TruePeak::default),
            kept: ChannelLoudness { segment_ms: SEGMENT_MS, ..ChannelLoudness::default() },
        })
    }

    pub fn push(&mut self, sample: f32) {
        let weighted = self.high_pass.run(self.shelf.run(f64::from(sample)));
        self.sum += weighted * weighted;
        let peak = match self.oversample.as_mut() {
            Some(oversample) => oversample.push(sample),
            None => sample.abs(),
        };
        self.peak = self.peak.max(peak);
        self.in_segment += 1;
        if self.in_segment == self.segment_samples {
            self.close_segment();
        }
    }

    fn close_segment(&mut self) {
        if self.in_segment > 0 {
            self.kept.energies.push((self.sum / self.in_segment as f64) as f32);
            self.kept.true_peaks.push(self.peak);
        }
        self.in_segment = 0;
        self.sum = 0.0;
        self.peak = 0.0;
    }

    /// What was measured; a last, shorter segment counts too.
    pub fn finish(mut self) -> ChannelLoudness {
        self.close_segment();
        self.kept
    }
}

/// Loudness of a mean square of the K-weighted signal (BS.1770: -0.691 + 10 log10).
pub fn lufs(energy: f64) -> f64 {
    if energy <= 0.0 {
        f64::NEG_INFINITY
    } else {
        -0.691 + 10.0 * energy.log10()
    }
}

/// The integrated loudness of a run of segments in order (a stretch of the program, already
/// multiplied by their gains): 400 ms blocks every 100 ms, gated at -70 LUFS and then 10 LU
/// below the loudness of the blocks above that. `None` when nothing passes the gates.
pub fn integrated(segments: &[f64]) -> Option<f64> {
    let blocks: Vec<f64> = segments.windows(4).map(|four| four.iter().sum::<f64>() / 4.0).collect();
    let above = |gate: f64| blocks.iter().copied().filter(move |block| lufs(*block) > gate);
    let mean = |values: Vec<f64>| (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);
    let absolute = mean(above(-70.0).collect())?;
    let relative = lufs(absolute) - 10.0;
    mean(above(relative).collect()).map(lufs)
}

/// Gain in dB as the factor of a mean square.
pub fn energy_gain(db: f64) -> f64 {
    10f64.powf(db / 10.0)
}

/// dB of a linear peak (dBTP for a true peak).
pub fn peak_db(peak: f64) -> f64 {
    if peak <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    }
}

/// One second order section, direct form I.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    /// BS.1770 stage 1, the high shelf of the head, for any rate (the 48 kHz coefficients of
    /// the standard come out of it).
    fn shelf(rate: f64) -> Self {
        let (f0, gain_db, q) = (1681.974_450_955_533, 3.999_843_853_973_347, 0.707_175_236_955_419_6);
        let k = (std::f64::consts::PI * f0 / rate).tan();
        let vh = 10f64.powf(gain_db / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        let a0 = 1.0 + k / q + k * k;
        Self::new(
            [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        )
    }

    /// BS.1770 stage 2, the RLB high pass.
    fn high_pass(rate: f64) -> Self {
        let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
        let k = (std::f64::consts::PI * f0 / rate).tan();
        let a0 = 1.0 + k / q + k * k;
        Self::new([1.0, -2.0, 1.0], [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0])
    }

    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Self { b, a, x: [0.0; 2], y: [0.0; 2] }
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// BS.1770-4 Annex 2: 4x oversampling with the 48 tap polyphase interpolator of the standard.
#[derive(Default)]
struct TruePeak {
    history: [f32; 12],
}

const PHASES: [[f32; 12]; 4] = [
    [
        0.001_708_984_375, 0.010_986_328_125, -0.019_653_320_312_5, 0.033_203_125, -0.059_448_242_187_5, 0.137_329_101_562_5,
        0.972_167_968_75, -0.102_294_921_875, 0.047_607_421_875, -0.026_611_328_125, 0.014_892_578_125, -0.008_300_781_25,
    ],
    [
        -0.029_174_804_687_5, 0.029_296_875, -0.051_757_812_5, 0.089_111_328_125, -0.166_503_906_25, 0.465_087_890_625,
        0.779_785_156_25, -0.200_317_382_812_5, 0.101_562_5, -0.058_227_539_062_5, 0.033_081_054_687_5, -0.018_920_898_437_5,
    ],
    [
        -0.018_920_898_437_5, 0.033_081_054_687_5, -0.058_227_539_062_5, 0.101_562_5, -0.200_317_382_812_5, 0.779_785_156_25,
        0.465_087_890_625, -0.166_503_906_25, 0.089_111_328_125, -0.051_757_812_5, 0.029_296_875, -0.029_174_804_687_5,
    ],
    [
        -0.008_300_781_25, 0.014_892_578_125, -0.026_611_328_125, 0.047_607_421_875, -0.102_294_921_875, 0.972_167_968_75,
        0.137_329_101_562_5, -0.059_448_242_187_5, 0.033_203_125, -0.019_653_320_312_5, 0.010_986_328_125, 0.001_708_984_375,
    ],
];

impl TruePeak {
    /// The highest of the four interpolated values after this sample (and the sample itself).
    fn push(&mut self, sample: f32) -> f32 {
        self.history.copy_within(0..11, 1);
        self.history[0] = sample;
        let mut peak = sample.abs();
        for phase in &PHASES {
            let value: f32 = phase.iter().zip(&self.history).map(|(tap, x)| tap * x).sum();
            peak = peak.max(value.abs());
        }
        peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(meter: &mut Meter, rate: u32, hz: f64, amplitude: f64, seconds: f64, phase: f64) {
        let count = (f64::from(rate) * seconds) as usize;
        for n in 0..count {
            let t = n as f64 / f64::from(rate);
            meter.push((amplitude * (2.0 * std::f64::consts::PI * hz * t + phase).sin()) as f32);
        }
    }

    fn integrated_of(kept: &ChannelLoudness) -> f64 {
        integrated(&kept.energies.iter().map(|e| f64::from(*e)).collect::<Vec<_>>()).unwrap()
    }

    /// EBU Tech 3341 style: a 1 kHz sine at -20 dBFS on one channel is -23 LUFS (+-0.1 LU),
    /// at 48 and at 44.1 kHz.
    #[test]
    fn a_minus_20_dbfs_sine_on_one_channel_is_minus_23_lufs() {
        for rate in [48_000, 44_100] {
            let mut meter = Meter::new(rate).unwrap();
            sine(&mut meter, rate, 997.0, 0.1, 20.0, 0.0);
            let measured = integrated_of(&meter.finish());
            assert!((measured + 23.0).abs() < 0.1, "{rate} Hz: {measured}");
        }
    }

    /// Silence does not pull the integrated loudness down (absolute gate), nor sound 20 LU
    /// under the rest (relative gate).
    #[test]
    fn the_gates_leave_out_silence_and_much_quieter_sound() {
        let mut meter = Meter::new(48_000).unwrap();
        sine(&mut meter, 48_000, 997.0, 0.1, 10.0, 0.0);
        sine(&mut meter, 48_000, 997.0, 0.0, 10.0, 0.0);
        sine(&mut meter, 48_000, 997.0, 0.01, 10.0, 0.0);
        let measured = integrated_of(&meter.finish());
        assert!((measured + 23.0).abs() < 0.15, "{measured}");
    }

    /// At 48 kHz the filters are the coefficients printed in BS.1770-4.
    #[test]
    fn at_48_khz_the_k_filter_is_the_one_of_the_standard() {
        let shelf = Biquad::shelf(48_000.0);
        let high = Biquad::high_pass(48_000.0);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(near(shelf.b[0], 1.535_124_859_586_97) && near(shelf.b[1], -2.691_696_189_406_38) && near(shelf.b[2], 1.198_392_810_852_85));
        assert!(near(shelf.a[0], -1.690_659_293_182_41) && near(shelf.a[1], 0.732_480_774_215_85));
        assert!(near(high.a[0], -1.990_047_454_833_98) && near(high.a[1], 0.990_072_250_366_21));
    }

    /// A gain changes the loudness by as many dB.
    #[test]
    fn a_gain_on_the_segments_moves_the_loudness_by_as_many_db() {
        let mut meter = Meter::new(48_000).unwrap();
        sine(&mut meter, 48_000, 997.0, 0.1, 10.0, 0.0);
        let kept = meter.finish();
        let louder: Vec<f64> = kept.energies.iter().map(|e| f64::from(*e) * energy_gain(6.0)).collect();
        assert!((integrated(&louder).unwrap() - integrated_of(&kept) - 6.0).abs() < 0.01);
        assert_eq!(integrated(&[0.0; 40]), None, "silence has no loudness");
    }

    /// A quarter-rate sine sampled 45 degrees off its crest: the samples show -3 dB of the
    /// crest, the true peak finds it.
    #[test]
    fn the_true_peak_finds_the_crest_between_samples() {
        let mut meter = Meter::new(48_000).unwrap();
        sine(&mut meter, 48_000, 12_000.0, 0.5, 1.0, std::f64::consts::FRAC_PI_4);
        let kept = meter.finish();
        let peak = kept.true_peaks.iter().fold(0f32, |a, b| a.max(*b));
        assert!((peak_db(f64::from(peak)) - peak_db(0.5)).abs() < 0.6, "{peak}");
        assert!(peak > 0.5 * 0.75, "above the samples ({})", 0.5 * std::f32::consts::FRAC_1_SQRT_2);
        assert_eq!(kept.energies.len(), 10);
        kept.validate().unwrap();
    }
}
