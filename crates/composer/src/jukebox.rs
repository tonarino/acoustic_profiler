use eyre::Result;
use rodio::{
    source::{Buffered, SamplesConverter},
    Decoder, OutputStreamHandle, Source,
};
use std::{fs::File, io::BufReader, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Sample {
    Click,
}

impl Sample {
    const ALL: &[Sample] = &[Self::Click];

    fn filename(&self) -> &'static Path {
        Path::new(match self {
            Sample::Click => "click.wav",
        })
    }
}

type Buffer = Buffered<SamplesConverter<Decoder<BufReader<File>>, f32>>;

/// Records (samples) are loaded to the jukebox once, and then in can quickly play any of them.
pub struct Jukebox {
    samples: Vec<(Sample, Buffer)>,
}

impl Jukebox {
    pub(crate) fn new() -> Result<Self> {
        let base_path = Path::new("src/sound_samples");
        let samples = Sample::ALL
            .iter()
            .map(|&sample| -> Result<(Sample, Buffer)> {
                let path = base_path.join(sample.filename());
                let file = BufReader::new(File::open(path)?);
                let source = Decoder::new(file)?;

                Ok((sample, source.convert_samples().buffered()))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { samples })
    }

    pub(crate) fn play(&self, output_stream: &OutputStreamHandle, sample: Sample) -> Result<()> {
        let (_, buffer) = self
            .samples
            .iter()
            .find(|(s, _)| *s == sample)
            .expect("programmer error, all possible samples should be loaded");

        output_stream.play_raw(buffer.clone())?;
        Ok(())
    }
}
