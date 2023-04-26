use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, StreamConfig,
};
use eyre::Result;
use fundsp::hacker::*;

pub struct SoundController {
    _stream: cpal::Stream,
    custom_organ_hz: Shared<f64>,
}

impl SoundController {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();

        let device = host.default_output_device().expect("Failed to find a default output device");

        // TODO(bschwind) - Hardcode this for now, but let's extract these param
        //                  from device.default_output_config later.
        let stream_config = StreamConfig {
            channels: 1,
            sample_rate: SampleRate(48_000),
            buffer_size: BufferSize::Default,
        };

        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        let custom_organ_hz = shared(0.0f64);
        let custom_osc = var_fn(&custom_organ_hz, |hz| hz.clamp(0.0, 1000.0))
            >> organ()
            >> chorus(0, 0.0, 0.1, 0.1);

        let mut dsp_graph = 0.3
            * (custom_osc
                + organ_hz(midi_hz(57.0))
                + organ_hz(midi_hz(61.0))
                + organ_hz(midi_hz(64.0)));

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data {
                    *sample = dsp_graph.get_mono() as f32;
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;

        Ok(Self { _stream: stream, custom_organ_hz })
    }

    pub fn increment_hz(&mut self) {
        let current_val = self.custom_organ_hz.value();
        self.custom_organ_hz.set_value(current_val + 5.0);
    }
}
