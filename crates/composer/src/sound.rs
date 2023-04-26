use cpal::{
    traits::{DeviceTrait, HostTrait},
    SampleFormat,
};
use eyre::{bail, eyre, Result};
use rodio::{dynamic_mixer::DynamicMixerController, Source};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) struct AudioOutput {
    mixer_controller: Arc<DynamicMixerController<f32>>,
    /// UNIX timestamp in nanoseconds, as an atomic variable.
    last_callback_timestamp_ns: Arc<AtomicU64>,
    _stream: cpal::Stream,
}

impl AudioOutput {
    pub(crate) fn new() -> Result<Self> {
        let cpal_device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| eyre!("no cpal audio output device found"))?;
        let supported_config = cpal_device.default_output_config()?;
        let stream_config = supported_config.config();
        println!(
            "Using audio device '{}', supported config {:?}, stream config {:?}.",
            cpal_device.name()?,
            supported_config,
            stream_config,
        );
        if supported_config.sample_format() != SampleFormat::F32 {
            bail!("Only F32 sample format supported for now.");
        }

        let (mixer_controller, mut mixer) =
            rodio::dynamic_mixer::mixer::<f32>(stream_config.channels, stream_config.sample_rate.0);

        let last_callback_timestamp_ns = Arc::new(AtomicU64::new(0));

        let last_callback_timestamp_ns_alias = Arc::clone(&last_callback_timestamp_ns);
        let _stream = cpal_device.build_output_stream::<f32, _, _>(
            &stream_config,
            move |data_out, _info| {
                // println!("data_callback: date_out.len(): {}, info: {_info:?}.", data_out.len());

                // TODO(Matej): can we use timestamps in _info instead?
                // TODO(Matej): specify less strict ordering (in all places for this atomic var)
                last_callback_timestamp_ns_alias.store(current_timestamp_ns(), Ordering::SeqCst);

                data_out.iter_mut().for_each(|d| *d = mixer.next().unwrap_or(0f32))
            },
            |err| eprintln!("Got cpal stream error callback: {err}."),
            None,
        )?;

        Ok(Self { mixer_controller, last_callback_timestamp_ns, _stream })
    }

    pub(crate) fn play<S: Source<Item = f32> + Send + 'static>(&self, source: S) {
        let ns_since_last_callback = current_timestamp_ns()
            .saturating_sub(self.last_callback_timestamp_ns.load(Ordering::SeqCst));
        // println!("ns_since_last_callback: {ns_since_last_callback:>9}.",);

        // We assume that cpal's audio callbacks come at a steady rate, so we delay sounds to play
        // by the time since last audio callback, so that it gets played at correct offset during
        // the *next* callback. This is needed to accurately position sounds (sub buffer level).
        // https://github.com/tonarino/acoustic_profiler/issues/19#issuecomment-1522348735
        self.mixer_controller.add(source.delay(Duration::from_nanos(ns_since_last_callback)));
    }
}

fn current_timestamp_ns() -> u64 {
    let duration_since_epoch =
        SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get current UNIX time");
    duration_since_epoch
        .as_nanos()
        .try_into()
        .expect("Cannot convert current UNIX timestamp in nanoseconds into u64")
}
