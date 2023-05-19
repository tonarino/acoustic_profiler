use crate::util::current_timestamp;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    OutputCallbackInfo, OutputStreamTimestamp, SampleFormat,
};
use eyre::{bail, eyre, Result};
use rodio::{
    dynamic_mixer::{DynamicMixer, DynamicMixerController},
    Source,
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Arc,
    },
    time::Duration,
};

pub(crate) struct AudioOutput {
    source_tx: Sender<TimedSource>,
    play_delay: Duration,
    too_early_plays: Arc<AtomicU64>,
    _stream: cpal::Stream,
}

/// Abstraction to actually produce sound using the [AudioOutput::play()] method.
/// Uses `cpal` and `rodio` behind the curtains. Great care is taken to position played samples
/// precisely in time so that sound superposition works well even at high frequencies.
/// Playback stops when this struct is dropped.
impl AudioOutput {
    pub(crate) fn new(play_delay: Duration) -> Result<Self> {
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

        let (mixer_controller, mixer) =
            rodio::dynamic_mixer::mixer::<f32>(stream_config.channels, stream_config.sample_rate.0);

        let (source_tx, source_rx) = channel();

        let too_early_plays = Arc::default();

        // The mixer_controller can be shared between threads, but we want to precisely control
        // when we add new sources w.r.t. the audio callback, so we move it to the audio thread and
        // use a mpsc channel to send new sources to the audio thread.
        let mut audio_callback =
            AudioCallback::new(mixer_controller, mixer, source_rx, &too_early_plays);
        let _stream = cpal_device.build_output_stream::<f32, _, _>(
            &stream_config,
            move |data_out, info| audio_callback.fill_data(data_out, info),
            |err| eprintln!("Got cpal stream error callback: {err}."),
            None,
        )?;

        Ok(Self { source_tx, play_delay, too_early_plays, _stream })
    }

    pub(crate) fn play<S>(&self, source: S, timestamp: Duration)
    where
        S: Source<Item = f32> + Send + 'static,
    {
        let play_at_timestamp = timestamp + self.play_delay;

        // TODO(Matej): we are in fact double-boxing because DynamicMixerController internally adds
        // another box. But we need a sized type to send it through threads. We could make this
        // method non-generic, but that would be less flexible, so just accept it for now.
        let source = Box::new(source);

        self.source_tx
            .send(TimedSource { source, play_at_timestamp })
            .expect("source receiver should be still alive");
    }

    /// Get "too early plays" counter since the last call of this method.
    pub(crate) fn fetch_too_early_plays(&self) -> u64 {
        self.too_early_plays.swap(0, Ordering::SeqCst)
    }
}

/// An f32 [rodio::source::Source] with UNIX timestamp of desired play time attached.
struct TimedSource {
    source: Box<dyn Source<Item = f32> + Send + 'static>,
    play_at_timestamp: Duration,
}

/// A sort of manual implementation of the closure used as cpal audio data callback, for tidiness.
struct AudioCallback {
    mixer_controller: Arc<DynamicMixerController<f32>>,
    mixer: DynamicMixer<f32>,
    source_rx: Receiver<TimedSource>,
    too_early_plays: Arc<AtomicU64>,
}

impl AudioCallback {
    fn new(
        mixer_controller: Arc<DynamicMixerController<f32>>,
        mixer: DynamicMixer<f32>,
        source_rx: Receiver<TimedSource>,
        too_early_plays: &Arc<AtomicU64>,
    ) -> Self {
        let too_early_plays = Arc::clone(too_early_plays);
        Self { mixer_controller, mixer, source_rx, too_early_plays }
    }

    fn fill_data(&mut self, data_out: &mut [f32], info: &OutputCallbackInfo) {
        let now = current_timestamp();
        // cpal gives us two timestamps that cannot be compared to unix time directly as they have a different epoch.
        // However subtracting them gives us the duration between when we were called (i.e. now) and when the buffer
        // we produce will be played.
        let OutputStreamTimestamp { playback, callback } = info.timestamp();
        let playback_delay =
            playback.duration_since(&callback).expect("playback shouldn't be planned in past");
        // ...and by adding it to current unix timestamp we get a unix timestamp of the instant the buffer will be played.
        let playback_unix_timestamp = now + playback_delay;

        // Add possible new sources to the list
        loop {
            match self.source_rx.try_recv() {
                Ok(timed_source) => {
                    let delay = timed_source
                        .play_at_timestamp
                        .checked_sub(playback_unix_timestamp)
                        .unwrap_or_else(|| {
                            self.too_early_plays.fetch_add(1, Ordering::SeqCst);
                            Duration::ZERO
                        });
                    self.mixer_controller.add(timed_source.source.delay(delay));
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => panic!("source sender should be still alive"),
            }
        }

        data_out.iter_mut().for_each(|d| *d = self.mixer.next().unwrap_or(0f32))
    }
}
