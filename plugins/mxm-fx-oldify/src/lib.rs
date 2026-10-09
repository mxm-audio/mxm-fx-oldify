//! `mxm-fx-oldify` — the collection's effect that makes sound old: cassette tape and old records.
//!
//! The cassette and record engines are not built yet. What ships today is the shell every MXM
//! plugin has, around the DSP crate's placeholder chain: Drive, Width, Output and Mix.

macro_rules! plugin_name {
    () => {
        "mxm-fx-oldify"
    };
}

pub const NAME: &str = plugin_name!();
pub const CLAP_ID: &str = concat!("dk.mxm.", plugin_name!());

pub mod editor;
pub mod params;
pub mod preset;
pub mod telemetry;

use std::sync::Arc;

use mxm_fx_oldify_dsp::Oldify;
use nice_plug::prelude::*;
use params::MxmFxOldifyParams;
use telemetry::Telemetry;

pub struct MxmFxOldify {
    pub params: Arc<MxmFxOldifyParams>,
    telemetry: Arc<Telemetry>,
    engine: Oldify,
    input_channels: usize,
}

impl Default for MxmFxOldify {
    fn default() -> Self {
        Self {
            params: Arc::new(MxmFxOldifyParams::default()),
            telemetry: Telemetry::shared(),
            engine: Oldify::new(48_000.0).expect("48 kHz is supported"),
            input_channels: 1,
        }
    }
}

impl MxmFxOldify {
    /// A fresh engine at `sample_rate`, settled at the controls as they stand, so an activation
    /// never ramps from the identity to a restored state.
    fn prepare(&mut self, sample_rate: f32, input_channels: usize) -> bool {
        let Ok(mut engine) = Oldify::new(sample_rate) else {
            return false;
        };
        engine.set_controls(self.params.controls());
        engine.reset();
        self.engine = engine;
        self.input_channels = input_channels.clamp(1, 2);
        true
    }

    pub fn prepare_for_test(&mut self, sample_rate: f32, input_channels: usize) -> bool {
        self.prepare(sample_rate, input_channels)
    }

    pub fn process_block_for_test(&mut self, channels: &mut [&mut [f32]]) -> ProcessStatus {
        self.process_block(channels)
    }

    fn process_block(&mut self, channels: &mut [&mut [f32]]) -> ProcessStatus {
        let Some(first) = channels.first() else {
            return ProcessStatus::Normal;
        };
        let samples = first.len();
        let peak = |channel: &[f32]| {
            channel
                .iter()
                .filter(|sample| sample.is_finite())
                .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
        };
        let inputs = self.input_channels.min(channels.len());
        let input_peak = channels[..inputs]
            .iter()
            .fold(0.0f32, |all, channel| all.max(peak(&channel[..samples])));

        // A mono input feeds both sides of the stereo output.
        let output_peak = match channels {
            [left, right, ..] => {
                if self.input_channels == 1 {
                    right[..samples].copy_from_slice(&left[..samples]);
                }
                self.engine.process(
                    &mut left[..samples],
                    &mut right[..samples],
                    self.params.controls(),
                );
                peak(&left[..samples]).max(peak(&right[..samples]))
            }
            // Every declared layout has a stereo output, so a host never hands over fewer.
            _ => 0.0,
        };
        self.telemetry.publish(input_peak, output_peak);

        // No state outlives the input yet, so there is never a tail to report.
        ProcessStatus::Normal
    }
}

impl Plugin for MxmFxOldify {
    const NAME: &'static str = NAME;
    const VENDOR: &'static str = "mxm";
    const URL: &'static str = "https://mxm.dk";
    const EMAIL: &'static str = "plugins@mxm.dk";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = editor::MxmFxOldifyEditor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.telemetry.clone())
    }

    fn activate(
        &mut self,
        layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.prepare(
            config.sample_rate,
            layout
                .main_input_channels
                .map_or(1, |channels| channels.get() as usize),
        )
    }

    fn reset(&mut self) {
        // A host resets without a callback between (a bypass, a transport restart), and a
        // parameter flush may have moved a control meanwhile: start at the controls as they stand.
        self.engine.set_controls(self.params.controls());
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.process_block(buffer.as_slice())
    }
}

impl ClapPlugin for MxmFxOldify {
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Makes sound old: cassette tape and old records, from a new deck to a worn 78");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Stereo,
    ];
}

nice_export_clap!(MxmFxOldify);

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::{InternalParamMut, Param};

    fn set(param: &FloatParam, value: f32) {
        unsafe {
            let _ = param._internal_set_normalized_value(param.preview_normalized(value));
        }
    }

    fn programme(frames: usize, step: f32) -> Vec<f32> {
        (0..frames).map(|n| (n as f32 * step).sin() * 0.4).collect()
    }

    #[test]
    fn permanent_identity_and_layouts_are_fixed() {
        assert_eq!(NAME, "mxm-fx-oldify");
        assert_eq!(CLAP_ID, "dk.mxm.mxm-fx-oldify");
        mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
        assert_eq!(MxmFxOldify::AUDIO_IO_LAYOUTS.len(), 2);
        assert_eq!(MxmFxOldify::MIDI_INPUT, MidiConfig::None);
    }

    /// **The defaults pass the input to the bit** in both layouts, a mono input reaching both
    /// sides: until the engines exist, an inserted Oldify is out of the way.
    #[test]
    fn the_defaults_pass_the_input_to_the_bit_in_both_layouts() {
        for inputs in [1, 2] {
            let mut plugin = MxmFxOldify::default();
            assert!(plugin.prepare(48_000.0, inputs));
            let source_l = programme(4096, 0.13);
            let source_r = programme(4096, 0.07);
            let mut left = source_l.clone();
            let mut right = source_r.clone();
            let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
            let status = plugin.process_block(&mut channels);
            assert!(matches!(status, ProcessStatus::Normal));
            assert_eq!(left, source_l);
            assert_eq!(right, if inputs == 1 { source_l } else { source_r });
        }
    }

    #[test]
    fn mix_zero_is_dry_to_the_bit_whatever_else_is_set() {
        let mut plugin = MxmFxOldify::default();
        set(&plugin.params.drive, 15.0);
        set(&plugin.params.width, 0.0);
        set(&plugin.params.output, 0.1);
        set(&plugin.params.mix, 0.0);
        assert!(plugin.prepare(48_000.0, 2));
        let source_l = programme(4096, 0.11);
        let source_r = programme(4096, 0.05);
        let mut left = source_l.clone();
        let mut right = source_r.clone();
        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
        plugin.process_block(&mut channels);
        assert_eq!(left, source_l);
        assert_eq!(right, source_r);
    }

    /// **A control moved between blocks ramps**, so automation does not step the audio, and the
    /// chain is heard: Drive up 12 dB doubles and doubles again what comes out.
    #[test]
    fn a_moved_control_ramps_and_is_heard() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 2));
        set(&plugin.params.drive, mxm_fx_oldify_dsp::db_to_gain(12.0));
        let source: Vec<f32> = vec![0.25; 4096];
        let mut left = source.clone();
        let mut right = source.clone();
        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
        plugin.process_block(&mut channels);
        assert!(left[0] < 0.26, "Drive jumped: {}", left[0]);
        let mut largest_step = 0.0f32;
        for pair in left.windows(2) {
            largest_step = largest_step.max((pair[1] - pair[0]).abs());
        }
        assert!(largest_step < 0.01, "Drive stepped by {largest_step}");
        let gain = left[4095] / 0.25;
        assert!((gain - 3.981).abs() < 0.01, "12 dB came out as ×{gain}");
    }

    /// **A reset starts at the controls as they stand**, not ramping to them from wherever the
    /// previous playback left off.
    #[test]
    fn a_reset_starts_at_the_controls_as_they_stand() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 2));
        set(&plugin.params.output, 0.5);
        plugin.reset();
        let mut left = vec![0.5f32; 16];
        let mut right = vec![0.5f32; 16];
        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
        plugin.process_block(&mut channels);
        let expected = 0.5 * plugin.params.output.value();
        assert!(left.iter().all(|sample| *sample == expected), "{left:?}");
    }

    #[test]
    fn levels_reach_the_editor() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 1));
        set(&plugin.params.output, mxm_fx_oldify_dsp::db_to_gain(-6.0));
        plugin.reset();
        let mut left = vec![0.8f32; 256];
        let mut right = vec![0.0f32; 256];
        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
        plugin.process_block(&mut channels);
        let [input, output] = plugin.telemetry.take_levels();
        assert_eq!(input, 0.8);
        assert!((output - 0.4).abs() < 0.01, "{output}");
    }
}

/// What a player reads — on hover in the editor, and in a host's plugin browser — speaks to the
/// player about the sound, never about the machine or the code (`mxm_plugin_test::hover_text`).
#[cfg(test)]
mod speaks_to_the_player {
    #[test]
    fn hover_text() {
        mxm_plugin_test::hover_text::speaks_to_the_player(env!("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn host_description() {
        mxm_plugin_test::hover_text::host_description_speaks_to_the_player(env!(
            "CARGO_MANIFEST_DIR"
        ));
    }
}
