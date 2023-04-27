use cpal::{
    traits::{DeviceTrait, HostTrait},
    OutputCallbackInfo, OutputStreamTimestamp, SampleFormat, StreamInstant,
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
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) struct AudioOutput {
    source_tx: Sender<TimedSource>,
    play_delay: Duration,
    too_early_plays: Arc<AtomicU64>,
    _stream: cpal::Stream,
}

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

    pub(crate) fn play<S: Source<Item = f32> + Send + 'static>(&self, source: S) {
        // TODO(Matej): use timestamp from the event itself once we have it.
        let play_at_timestamp = current_timestamp() + self.play_delay;

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
    stream_start: Option<StreamStart>,
}

impl AudioCallback {
    fn new(
        mixer_controller: Arc<DynamicMixerController<f32>>,
        mixer: DynamicMixer<f32>,
        source_rx: Receiver<TimedSource>,
        too_early_plays: &Arc<AtomicU64>,
    ) -> Self {
        let too_early_plays = Arc::clone(too_early_plays);
        Self { mixer_controller, mixer, source_rx, too_early_plays, stream_start: None }
    }

    fn fill_data(&mut self, data_out: &mut [f32], info: &OutputCallbackInfo) {
        let stream_timestamp = info.timestamp();
        let stream_start =
            self.stream_start.get_or_insert_with(|| StreamStart::new(stream_timestamp));

        // At least on Linux ALSA, cpal gives very strange stream timestamp on very first call.
        if stream_start.callback > stream_timestamp.callback {
            eprintln!("cpal's stream timestamp jumped backwards, resetting stream start.");
            *stream_start = StreamStart::new(stream_timestamp);
        }

        // UNIX timestamp of when the buffer filled during this call will be actually played.
        let buffer_playback_timestamp = stream_start.realtime
            + (stream_timestamp.playback.duration_since(&stream_start.callback).expect(
                "current playback timestamp should be larger than start callback timestamp",
            ));

        // Add possible new sources to the list
        loop {
            match self.source_rx.try_recv() {
                Ok(timed_source) => {
                    let delay = timed_source
                        .play_at_timestamp
                        .checked_sub(buffer_playback_timestamp)
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

/// An utility struct that anchors stream's internal timestamp to real world UNIX timestamp.
struct StreamStart {
    /// Timestamp of the start of stream _playback_ as the `Duration` since the UNIX epoch.
    realtime: Duration,
    /// Internal cpal's callback timestamp of stream start.
    callback: StreamInstant,
}

impl StreamStart {
    fn new(stream_timestamp: OutputStreamTimestamp) -> Self {
        let realtime = current_timestamp();
        println!(
            "Audio stream starting at {realtime:?} UNIX timestamp, callback timestamp {:?}, \
            playback timestamp delayed by {:?} after callback.",
            stream_timestamp.callback,
            stream_timestamp.playback.duration_since(&stream_timestamp.callback)
        );

        // We record _callback_ timestamp for the purposes of fixing playback in real time.
        Self { realtime, callback: stream_timestamp.callback }
    }
}

// Get current timestamp as the `Duration` since the UNIX epoch.
fn current_timestamp() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get current UNIX time")
}
