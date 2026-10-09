//! Lock-free lossy audio-to-editor telemetry: the input and output peaks, the output's clip, and
//! the host tempo in force, so a synced rate reads the division actually sounding. This effect has
//! no note port, so it has no developer MIDI channel and no request slots.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub struct Telemetry {
    input: AtomicU32,
    output: AtomicU32,
    clipped: AtomicBool,
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
            input: AtomicU32::new(0),
            output: AtomicU32::new(0),
            clipped: AtomicBool::new(false),
            tempo: mxm_tempo::TempoCell::new(),
        }
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// A peak is max-combined until the editor takes it, so a frame the editor missed cannot hide
    /// a transient.
    fn publish_max(slot: &AtomicU32, value: f32) {
        let value = if value.is_finite() { value.abs() } else { 0.0 };
        let mut current = slot.load(Ordering::Relaxed);
        while f32::from_bits(current) < value {
            match slot.compare_exchange_weak(
                current,
                value.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(seen) => current = seen,
            }
        }
    }

    /// One block's peaks. A clip latches until the player acknowledges it (design system §5.4).
    pub fn publish(&self, input: f32, output: f32, tempo: Option<f64>) {
        self.tempo.publish(tempo);
        Self::publish_max(&self.input, input);
        Self::publish_max(&self.output, output);
        if output >= 1.0 {
            self.clipped.store(true, Ordering::Relaxed);
        }
    }

    /// The input and output peaks since the last take, and resets both.
    pub fn take_levels(&self) -> [f32; 2] {
        [
            f32::from_bits(self.input.swap(0, Ordering::Relaxed)),
            f32::from_bits(self.output.swap(0, Ordering::Relaxed)),
        ]
    }

    /// The host tempo last published, if it was one.
    pub fn tempo(&self) -> Option<f64> {
        self.tempo.get()
    }

    /// No tempo until the next block reports one: what an activation leaves.
    pub fn forget_tempo(&self) {
        self.tempo.publish(None);
    }

    pub fn clipped(&self) -> bool {
        self.clipped.load(Ordering::Relaxed)
    }

    pub fn clear_clip(&self) {
        self.clipped.store(false, Ordering::Relaxed);
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
        assert!(telemetry.clipped());
        telemetry.clear_clip();
        assert!(!telemetry.clipped());
        assert_eq!(telemetry.tempo(), Some(90.0));
        telemetry.forget_tempo();
        assert_eq!(telemetry.tempo(), None);
    }
}
