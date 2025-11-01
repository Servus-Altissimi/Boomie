#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveformType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    Noise,
}

impl WaveformType {
    pub fn generate_sample(&self, phase: f32) -> f32 { // Phase should be in the range [0.0, 1.0)
        match self {
            WaveformType::Sine => (phase * std::f32::consts::TAU).sin(),
            WaveformType::Square => if (phase * 2.0) % 1.0 < 0.5 { 1.0 } else { -1.0 },
            WaveformType::Sawtooth => (phase * 2.0) % 1.0 * 2.0 - 1.0,
            WaveformType::Noise => fastrand::f32() * 2.0 - 1.0, // WaveformType::Noise => (((phase * 1235.647).sin() * 43758.5453).fract() * 2.0 - 1.0), <- possible replacement
            WaveformType::Triangle => {
                let p = (phase * 2.0) % 1.0;
                if p < 0.5 { p * 4.0 - 1.0 } else { 3.0 - p * 4.0 }
            }
        }
    }
}