use crate::audio_output::AudioOutput;
use eyre::{Context, Result};
use rodio::{
    source::{Buffered, SamplesConverter},
    Decoder, Source,
};
use std::{collections::HashMap, fs::File, io::BufReader, path::Path, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Sample {
    Click,
    Clack,
}

impl Sample {
    const ALL: &[Sample] = &[Self::Click, Sample::Clack];

    fn filename(&self) -> &'static Path {
        Path::new(match self {
            Sample::Click => "click.wav",
            Sample::Clack => "clack.wav",
        })
    }
}

type Buffer = Buffered<SamplesConverter<Decoder<BufReader<File>>, f32>>;

/// Records (samples) are loaded to the jukebox once, and then in can quickly play any of them.
pub struct Jukebox {
    samples: HashMap<Sample, Buffer>,
}

impl Jukebox {
    pub(crate) fn new() -> Result<Self> {
        let base_path = Path::new("src/sound_samples");
        let samples = Sample::ALL
            .iter()
            .map(|&sample| -> Result<(Sample, Buffer)> {
                let path = base_path.join(sample.filename());
                let file =
                    BufReader::new(File::open(&path).with_context(|| format!("opening {path:?}"))?);
                let source = Decoder::new(file).with_context(|| format!("decoding {path:?}"))?;

                Ok((sample, source.convert_samples().buffered()))
            })
            .collect::<Result<_>>()
            .context("loading records")?;

        Ok(Self { samples })
    }

    pub(crate) fn play(&self, audio_output: &AudioOutput, sample: Sample, timestamp: Duration) {
        let buffer = self
            .samples
            .get(&sample)
            .expect("programmer error, all possible samples should be loaded");

        audio_output.play(buffer.clone(), timestamp);
    }
}
