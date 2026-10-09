//! `mxm-fx-oldify` — the collection's effect that makes sound old: cassette tape and old records.
//!
//! The cassette engine is in; the record engine is to come (Record passes the sound, delayed by the
//! latency). The DSP is `mxm-fx-oldify-dsp`; this shell converts host state and units, reports the
//! chain's fixed latency and its tail, and publishes levels to the editor.

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

use mxm_fx_oldify_dsp::{Oldify, SILENCE};
use nice_plug::prelude::*;
use params::MxmFxOldifyParams;
use telemetry::Telemetry;

pub struct MxmFxOldify {
    pub params: Arc<MxmFxOldifyParams>,
    telemetry: Arc<Telemetry>,
    engine: Oldify,
    input_channels: usize,
    /// Samples since the input was last above the DSP's [`SILENCE`], counted as the DSP counts
    /// them, so the tail reported is the DSP's own: `u64::MAX` when long silent.
    silent_for: u64,
}

impl Default for MxmFxOldify {
    fn default() -> Self {
        Self {
            params: Arc::new(MxmFxOldifyParams::default()),
            telemetry: Telemetry::shared(),
            engine: Oldify::new(48_000.0).expect("48 kHz is supported"),
            input_channels: 1,
            silent_for: u64::MAX,
        }
    }
}

impl MxmFxOldify {
    /// A fresh engine at `sample_rate`, settled at the controls as they stand, so an activation
    /// never ramps from the identity to a restored state. Allocates (the engine's delay lines):
    /// called where the sample rate becomes known, never on the audio thread.
    fn prepare(&mut self, sample_rate: f32, input_channels: usize) -> bool {
        let Ok(mut engine) = Oldify::new(sample_rate) else {
            return false;
        };
        // No tempo until the first block reports one: a synced rate must not start at a division
        // resolved from a tempo the host may since have changed.
        self.telemetry.forget_tempo();
        engine.set_controls(self.params.controls(None));
        engine.reset();
        self.engine = engine;
        self.input_channels = input_channels.clamp(1, 2);
        self.silent_for = u64::MAX;
        true
    }

    /// The chain's delay, in samples, at the prepared sample rate: what activation reports.
    pub fn latency(&self) -> u32 {
        self.engine.latency()
    }

    pub fn prepare_for_test(&mut self, sample_rate: f32, input_channels: usize) -> bool {
        self.prepare(sample_rate, input_channels)
    }

    pub fn process_block_for_test(
        &mut self,
        channels: &mut [&mut [f32]],
        tempo: Option<f64>,
    ) -> ProcessStatus {
        self.process_block(channels, tempo)
    }

    /// One block at the host's `tempo`, which the synced rates resolve against.
    fn process_block(&mut self, channels: &mut [&mut [f32]], tempo: Option<f64>) -> ProcessStatus {
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
                self.count_silence(&left[..samples], &right[..samples]);
                self.engine.process(
                    &mut left[..samples],
                    &mut right[..samples],
                    self.params.controls(tempo),
                );
                peak(&left[..samples]).max(peak(&right[..samples]))
            }
            // Every declared layout has a stereo output, so a host never hands over fewer.
            _ => 0.0,
        };
        self.telemetry.publish(input_peak, output_peak, tempo);
        self.status()
    }

    /// Moves the silence count across this block's input, as the DSP will see it.
    fn count_silence(&mut self, left: &[f32], right: &[f32]) {
        let sounding = |sample: &f32| sample.is_finite() && sample.abs() > SILENCE;
        let last = left
            .iter()
            .zip(right)
            .rposition(|(l, r)| sounding(l) || sounding(r));
        self.silent_for = match last {
            Some(index) => (left.len() - 1 - index) as u64,
            None => self.silent_for.saturating_add(left.len() as u64),
        };
    }

    /// **The tail is the DSP's**: `KeepAlive` while it is endless (hiss that plays on in silence,
    /// so the host must never sleep the effect); otherwise `Tail(n)` with the samples left until the
    /// output is exact zero if the input stays silent from here, and `Normal` once it is, so a host
    /// may sleep the effect and doing nothing costs nothing.
    fn status(&self) -> ProcessStatus {
        if self.engine.endless() {
            return ProcessStatus::KeepAlive;
        }
        let left = u64::from(self.engine.tail()).saturating_sub(self.silent_for);
        if left == 0 {
            ProcessStatus::Normal
        } else {
            ProcessStatus::Tail(u32::try_from(left).unwrap_or(u32::MAX))
        }
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

    /// Builds the engine for the host's sample rate and **reports the chain's fixed latency**: the
    /// same for every control and every Medium, so it is reported here once and never changes
    /// while processing.
    fn activate(
        &mut self,
        layout: &AudioIOLayout,
        config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        let prepared = self.prepare(
            config.sample_rate,
            layout
                .main_input_channels
                .map_or(1, |channels| channels.get() as usize),
        );
        if prepared {
            context.set_latency_samples(self.engine.latency());
        }
        prepared
    }

    fn reset(&mut self) {
        // A host resets without a callback between (a bypass, a transport restart), and a
        // parameter flush may have moved a control meanwhile: start at the controls as they stand,
        // the synced rates at the last tempo seen, with every memory cleared.
        self.engine
            .set_controls(self.params.controls(self.telemetry.tempo()));
        self.engine.reset();
        self.silent_for = u64::MAX;
    }

    /// **A project saved before the tempo syncs** restores each Off rather than keeping this
    /// instance's, and a loaded preset's baseline gains it, so the preset stays clean
    /// (`mxm_preset::add_switches_off`).
    fn filter_state(state: &mut PluginState) {
        mxm_preset::add_switches_off(state, crate::preset::TEMPO_SYNC_IDS);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.process_block(buffer.as_slice(), context.transport().tempo)
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
    use std::cell::Cell;

    fn set(param: &FloatParam, value: f32) {
        unsafe {
            let _ = param._internal_set_normalized_value(param.preview_normalized(value));
        }
    }

    fn programme(frames: usize, step: f32) -> Vec<f32> {
        (0..frames).map(|n| (n as f32 * step).sin() * 0.4).collect()
    }

    fn run(plugin: &mut MxmFxOldify, left: &mut [f32], right: &mut [f32]) -> ProcessStatus {
        let mut channels: Vec<&mut [f32]> = vec![left, right];
        plugin.process_block(&mut channels, None)
    }

    /// An activation context that remembers the latency it was told.
    #[derive(Default)]
    struct Activation {
        latency: Cell<Option<u32>>,
    }

    impl ActivateContext<MxmFxOldify> for Activation {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute(&self, _task: ()) {}
        fn set_latency_samples(&self, samples: u32) {
            self.latency.set(Some(samples));
        }
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    #[test]
    fn permanent_identity_and_layouts_are_fixed() {
        assert_eq!(NAME, "mxm-fx-oldify");
        assert_eq!(CLAP_ID, "dk.mxm.mxm-fx-oldify");
        mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
        assert_eq!(MxmFxOldify::AUDIO_IO_LAYOUTS.len(), 2);
        assert_eq!(MxmFxOldify::MIDI_INPUT, MidiConfig::None);
    }

    /// **Activation reports the chain's latency** at the host's rate, as the engine has it.
    #[test]
    fn activation_reports_the_chains_latency_at_the_hosts_rate() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            let mut plugin = MxmFxOldify::default();
            let mut context = Activation::default();
            let config = BufferConfig {
                sample_rate: rate,
                min_buffer_size: None,
                max_buffer_size: 512,
                process_mode: ProcessMode::Realtime,
            };
            assert!(plugin.activate(&MxmFxOldify::AUDIO_IO_LAYOUTS[1], &config, &mut context));
            let reported = context.latency.get().expect("a latency was reported");
            assert_eq!(reported, Oldify::new(rate).unwrap().latency());
            assert_eq!(reported, plugin.latency());
        }
    }

    /// **An inserted Oldify is engaged**: the init patch is an audible cassette, finite, and a
    /// mono input feeds both sides exactly as a stereo pair of it would.
    #[test]
    fn the_defaults_are_audible_and_a_mono_input_feeds_both_sides() {
        let source = programme(16_384, 0.13);
        let mut mono = MxmFxOldify::default();
        assert!(mono.prepare(48_000.0, 1));
        let (mut left, mut right) = (source.clone(), vec![0.0; source.len()]);
        run(&mut mono, &mut left, &mut right);

        let mut stereo = MxmFxOldify::default();
        assert!(stereo.prepare(48_000.0, 2));
        let (mut pair_l, mut pair_r) = (source.clone(), source.clone());
        run(&mut stereo, &mut pair_l, &mut pair_r);

        assert_eq!(left, pair_l);
        assert_eq!(right, pair_r);
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
        let latency = mono.latency() as usize;
        let difference = (latency..source.len())
            .map(|n| (left[n] - source[n - latency]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            difference > 0.01,
            "the init patch is inaudible: {difference}"
        );
    }

    /// **Mix zero is the dry input, delayed by the reported latency**, whatever else is set.
    #[test]
    fn mix_zero_is_the_dry_input_delayed_by_the_latency() {
        let mut plugin = MxmFxOldify::default();
        set(&plugin.params.drive, 15.0);
        set(&plugin.params.width, 0.0);
        set(&plugin.params.output, 0.1);
        set(&plugin.params.saturation, 1.0);
        set(&plugin.params.mix, 0.0);
        assert!(plugin.prepare(48_000.0, 2));
        let latency = plugin.latency() as usize;
        let source_l = programme(8_192, 0.11);
        let source_r = programme(8_192, 0.05);
        let (mut left, mut right) = (source_l.clone(), source_r.clone());
        run(&mut plugin, &mut left, &mut right);
        assert!(left[..latency].iter().all(|sample| *sample == 0.0));
        assert_eq!(left[latency..], source_l[..8_192 - latency]);
        assert_eq!(right[latency..], source_r[..8_192 - latency]);
    }

    /// Every cassette control at its neutral setting: the chain is the input, delayed, through
    /// the oversampler's passband.
    fn neutral(plugin: &MxmFxOldify) {
        for (id, normalized) in [
            ("tape_tone", 0.0),
            ("saturation", 0.0),
            ("wow", 0.0),
            ("flutter", 0.0),
            ("hiss", 0.0),
            ("dropouts", 0.0),
            ("crosstalk", 0.0),
            ("age", 0.0),
            ("noise_follows", 0.0),
        ] {
            let (_, param, _) = plugin
                .params
                .param_map()
                .into_iter()
                .find(|(other, _, _)| *other == id)
                .expect("a parameter");
            unsafe {
                let _ = param._internal_set_normalized_value(normalized);
            }
        }
    }

    /// **A moved control ramps**, so automation does not step the audio, and is heard: Output up
    /// 12 dB on a neutral cassette.
    #[test]
    fn a_moved_control_ramps_and_is_heard() {
        let mut plugin = MxmFxOldify::default();
        neutral(&plugin);
        assert!(plugin.prepare(48_000.0, 2));
        // First the delayed input arrives at 0 dB, then Output moves while it plays.
        let frames = 2 * plugin.latency() as usize;
        let (mut left, mut right) = (vec![0.25f32; frames], vec![0.25f32; frames]);
        run(&mut plugin, &mut left, &mut right);
        assert!(
            (left[frames - 1] - 0.25).abs() < 1e-3,
            "{}",
            left[frames - 1]
        );
        set(&plugin.params.output, mxm_fx_oldify_dsp::db_to_gain(12.0));
        let (mut left, mut right) = (vec![0.25f32; frames], vec![0.25f32; frames]);
        run(&mut plugin, &mut left, &mut right);
        let largest_step = left
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(left[0] < 0.26, "Output jumped: {}", left[0]);
        assert!(largest_step < 0.01, "Output stepped by {largest_step}");
        let gain = left[frames - 1] / 0.25;
        assert!((gain - 3.981).abs() < 0.02, "12 dB came out as ×{gain}");
    }

    /// **A reset clears every memory and starts at the controls as they stand**: what was playing
    /// before it leaves nothing behind, so silence after it is exact silence at once.
    #[test]
    fn a_reset_leaves_nothing_of_what_played_before_it() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 2));
        let (mut left, mut right) = (programme(4_096, 0.2), programme(4_096, 0.3));
        run(&mut plugin, &mut left, &mut right);
        set(&plugin.params.output, 0.5);
        plugin.reset();
        let (mut left, mut right) = (vec![0.0f32; 8_192], vec![0.0f32; 8_192]);
        assert!(matches!(
            run(&mut plugin, &mut left, &mut right),
            ProcessStatus::Normal
        ));
        assert!(left.iter().chain(&right).all(|sample| *sample == 0.0));
    }

    /// **The tail is reported until the output is exact zero, then the effect may sleep**: `Tail`
    /// counts down across silence from the DSP's tail, and at its end the output is exact zero and
    /// the status `Normal`.
    #[test]
    fn the_tail_counts_down_to_exact_silence_and_then_the_effect_may_sleep() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 2));
        let tail = plugin.engine.tail();
        let (mut left, mut right) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        assert!(
            matches!(
                run(&mut plugin, &mut left, &mut right),
                ProcessStatus::Normal
            ),
            "a fresh effect is quiet"
        );

        let (mut left, mut right) = (programme(512, 0.2), programme(512, 0.3));
        assert!(matches!(
            run(&mut plugin, &mut left, &mut right),
            ProcessStatus::Tail(n) if n == tail
        ));

        let mut elapsed = 0u32;
        loop {
            let (mut left, mut right) = (vec![0.0f32; 512], vec![0.0f32; 512]);
            let status = run(&mut plugin, &mut left, &mut right);
            elapsed += 512;
            match status {
                ProcessStatus::Tail(n) => assert_eq!(n, tail.saturating_sub(elapsed)),
                ProcessStatus::Normal => {
                    assert!(elapsed >= tail, "Normal after {elapsed} of {tail}");
                    break;
                }
                _ => panic!("unexpected status"),
            }
        }
        let (mut left, mut right) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        run(&mut plugin, &mut left, &mut right);
        assert!(left.iter().chain(&right).all(|sample| *sample == 0.0));
    }

    /// **Hiss that plays on in silence keeps the effect alive**: with Noise stops in silence off,
    /// the status is `KeepAlive` — never `Normal`, never a finite tail — and silence in is hiss
    /// out. Turned back on after a long silence, the chain is quiet at once and the effect may
    /// sleep.
    #[test]
    fn an_endless_hiss_keeps_the_effect_alive_until_the_noise_stops() {
        let mut plugin = MxmFxOldify::default();
        unsafe {
            let _ = plugin
                .params
                .noise_stops
                ._internal_set_normalized_value(0.0);
        }
        assert!(plugin.prepare(48_000.0, 2));
        for _ in 0..20 {
            let (mut left, mut right) = (vec![0.0f32; 4_096], vec![0.0f32; 4_096]);
            assert!(matches!(
                run(&mut plugin, &mut left, &mut right),
                ProcessStatus::KeepAlive
            ));
        }
        let (mut left, mut right) = (vec![0.0f32; 4_096], vec![0.0f32; 4_096]);
        run(&mut plugin, &mut left, &mut right);
        assert!(
            left.iter().any(|sample| *sample != 0.0),
            "no hiss in silence"
        );

        unsafe {
            let _ = plugin
                .params
                .noise_stops
                ._internal_set_normalized_value(1.0);
        }
        let (mut left, mut right) = (vec![0.0f32; 4_096], vec![0.0f32; 4_096]);
        assert!(matches!(
            run(&mut plugin, &mut left, &mut right),
            ProcessStatus::Normal
        ));
        let (mut left, mut right) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        run(&mut plugin, &mut left, &mut right);
        assert!(left.iter().chain(&right).all(|sample| *sample == 0.0));
    }

    /// **The tempo in force reaches the editor, and an activation forgets it**, so a synced rate
    /// neither reads nor starts at a division from a tempo the host may since have changed.
    #[test]
    fn the_tempo_reaches_the_editor_and_an_activation_forgets_it() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 2));
        let (mut left, mut right) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
        plugin.process_block(&mut channels, Some(97.0));
        assert_eq!(plugin.telemetry.tempo(), Some(97.0));
        assert!(plugin.prepare(48_000.0, 2));
        assert_eq!(plugin.telemetry.tempo(), None);
    }

    /// **A reset resolves the syncs at the last tempo seen**: nothing resolved is cached, so a
    /// sync switched while the host held the effect still takes effect at the reset.
    #[test]
    fn a_reset_resolves_the_syncs_at_the_last_tempo_seen() {
        let mut plugin = MxmFxOldify::default();
        assert!(plugin.prepare(48_000.0, 2));
        let (mut left, mut right) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
        plugin.process_block(&mut channels, Some(120.0));
        unsafe {
            let _ = plugin.params.wow_sync._internal_set_normalized_value(1.0);
        }
        plugin.reset();
        let tempo = plugin.telemetry.tempo();
        assert_eq!(tempo, Some(120.0));
        assert_eq!(
            plugin.params.controls(tempo).cassette.wow.hz,
            params::WOW_SYNC
                .resolve(
                    true,
                    tempo,
                    plugin.params.wow_rate.unmodulated_normalized_value(),
                    0.1,
                    5.0
                )
                .map(|hz| hz as f32)
                .unwrap()
        );
    }

    #[test]
    fn levels_reach_the_editor() {
        let mut plugin = MxmFxOldify::default();
        set(&plugin.params.mix, 0.0);
        assert!(plugin.prepare(48_000.0, 1));
        let frames = plugin.latency() as usize + 256;
        let (mut left, mut right) = (vec![0.8f32; frames], vec![0.0f32; frames]);
        run(&mut plugin, &mut left, &mut right);
        let [input, output] = plugin.telemetry.take_levels();
        assert_eq!(input, 0.8);
        assert_eq!(output, 0.8, "Mix zero is the delayed input");
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
