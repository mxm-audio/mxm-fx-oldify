//! Lock-free lossy audio-to-editor telemetry: the input and output peaks, the output's clip, and
//! the host tempo in force, so a synced rate reads the division actually sounding. This effect has
//! no note port, so it has no developer MIDI channel and no request slots.

use std::sync::Arc;

pub struct Telemetry {
    /// The input level: one `mxm_telemetry::PeakMeter`, the collection's one meter policy.
    pub input: mxm_telemetry::PeakMeter,
    /// The output level and the clip light, which judges the output: one
    /// `mxm_telemetry::PeakMeter`, the collection's one meter policy.
    pub output: mxm_telemetry::PeakMeter,
    tempo: mxm_tempo::TempoCell,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            input: mxm_telemetry::PeakMeter::new(),
            output: mxm_telemetry::PeakMeter::new(),
            tempo: mxm_tempo::TempoCell::new(),
        }
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// One block's peaks. A clip latches until the player acknowledges it (design system §5.4).
    pub fn publish(&self, input: f32, output: f32, tempo: Option<f64>) {
        self.tempo.publish(tempo);
        self.input.publish(input);
        self.output.publish(output);
    }

    /// The input and output peaks since the last take, and resets both.
    pub fn take_levels(&self) -> [f32; 2] {
        [self.input.take(), self.output.take()]
    }

    /// The host tempo last published, if it was one.
    pub fn tempo(&self) -> Option<f64> {
        self.tempo.get()
    }

    /// No tempo until the next block reports one: what an activation leaves.
    pub fn forget_tempo(&self) {
        self.tempo.publish(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_are_max_combined_and_clip_latches() {
        let telemetry = Telemetry::new();
        telemetry.publish(0.2, 1.1, Some(120.0));
        telemetry.publish(0.7, 0.4, Some(90.0));
        assert_eq!(telemetry.take_levels(), [0.7, 1.1]);
        assert_eq!(telemetry.take_levels(), [0.0; 2]);
        assert!(telemetry.output.clipped());
        telemetry.output.clear_clip();
        assert!(!telemetry.output.clipped());
        assert_eq!(telemetry.tempo(), Some(90.0));
        telemetry.forget_tempo();
        assert_eq!(telemetry.tempo(), None);
    }
}
