use std::path::PathBuf;
use std::io::BufWriter;
use std::fs::File;
use std::sync::{Arc, Mutex};

use cpal::{FromSample, Sample};
use cpal::traits::DeviceTrait;
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

pub struct Buffer {
    model: PathBuf,
    data: Vec<f32>,
    pos: usize,
}

impl Buffer {
    pub fn new(model: PathBuf, size: usize) -> Buffer {
        Buffer {
            model,
            data: vec![0.0; size],
            pos: 0,
        }
    }

    pub fn push(
        & mut self,
        input: f32,
    ) {
        self.data[self.pos] = input;
        self.pos = self.pos + 1;

        if self.pos == self.data.len()-1 {
            self.pos = 0;
            self.transcribe();
        }
    }

    pub fn transcribe(&mut self) {

        let model_path = self.model.as_os_str();
        let context = WhisperContext::new_with_params(&model_path.to_str().unwrap(), WhisperContextParameters::default()).expect("Failed to load model.");

        let mut state = context.create_state().expect("Failed to create state.");

        let params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        
        state.full(params, &self.data[..]).expect("Failed to run model.");

        let n_segments = state.full_n_segments().expect("Failed to get number of segments");

        for i in 0..n_segments {
            println!("{}", state.full_get_segment_text(i).expect("Failed to get text."));
        }
        self.data = vec![0.0; self.data.len()];
    }
}

pub fn write_input_data<T, U>(
    input: &[T],
    writer: &Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>,
) where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                writer.write_sample(sample).ok();
            }
        }
    }
}

pub fn initialize_buffered_stream(device: cpal::Device, buffer: Arc<Mutex<Buffer>>, config: cpal::SupportedStreamConfig) -> Result<cpal::Stream, anyhow::Error> {

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    Ok(device.build_input_stream(&config.into(), move |data, _: &_| {
        for &sample in data.iter() {
            if let Ok(mut guard) = buffer.try_lock() {
                guard.push(sample);
            }
        }
    }, err_fn, None)?)

}


pub fn initialize_write_stream(device: cpal::Device, writer: Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>, config: cpal::SupportedStreamConfig) -> Result<cpal::Stream, anyhow::Error> {

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    match config.sample_format() {
        cpal::SampleFormat::I8 => Ok(device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i8, i8>(data, &writer),
            err_fn,
            None,
        )?),
        cpal::SampleFormat::I16 => Ok(device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i16, i16>(data, &writer),
            err_fn,
            None,
        )?),
        cpal::SampleFormat::I32 => Ok(device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i32, i32>(data, &writer),
            err_fn,
            None,
        )?),
        cpal::SampleFormat::F32 => Ok(device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<f32, f32>(data, &writer),
            err_fn,
            None,
        )?),
        sample_format => {
            Err(anyhow::Error::msg(format!(
                "Unsupported sample format '{sample_format}'"
            )))
        }
    }
}

