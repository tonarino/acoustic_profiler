use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, StreamConfig,
};
use eyre::Result;
use fundsp::hacker::*;
use std::sync::Arc;

const NUM_SLOTS: usize = 20;

pub struct SoundController {
    _stream: cpal::Stream,
    frontend_net: Net64,
    slots: Vec<NodeId>,
    slot_index: usize,
    click: Arc<Wave64>,
}

impl SoundController {
    pub fn new() -> Result<Self> {
        let click = Wave64::load("src/sound_samples/click.wav")?;

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

        let (frontend_net, mut backend, slots) = Self::build_dsp_graph();

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data {
                    *sample = backend.get_mono() as f32;
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;

        Ok(Self { _stream: stream, frontend_net, slots, slot_index: 0, click: Arc::new(click) })
    }

    fn build_dsp_graph() -> (Net64, Net64Backend, Vec<NodeId>) {
        let mut net = Net64::new(0, 1);

        // Create a node with 20 inputs that are mixed into one output
        let mixer = pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass()
            + pass();

        // add the mixer to the network and connect its output to the network's output
        let mixer_id = net.push(Box::new(mixer));
        net.connect_output(mixer_id, 0, 0);

        // Add 20 silent nodes to the network and connect each to one of the inputs of the mixer.
        let slots = (0..NUM_SLOTS)
            .map(|i| {
                let node_id = net.push(Box::new(zero()));
                net.connect(node_id, 0, mixer_id, i);
                node_id
            })
            .collect::<Vec<_>>();

        let backend = net.backend();

        (net, backend, slots)
    }

    pub fn play_click(&mut self) {
        let player = Wave64Player::new(&self.click, 0, 0, self.click.length(), None);
        let node_id = self.slots.get(self.slot_index).expect("programmer made a mistake");

        self.frontend_net.replace(*node_id, Box::new(An(player)));
        self.frontend_net.commit();

        self.slot_index = if self.slot_index == NUM_SLOTS - 1 { 0 } else { self.slot_index + 1 };
    }
}
