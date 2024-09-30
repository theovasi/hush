use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use clap::{Parser, Subcommand}; use cpal::traits::{DeviceTrait, StreamTrait};

use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

use hush::device::{get_input_device, list_input_devices};
use hush::utils::{Buffer, initialize_write_stream, initialize_buffered_stream};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Device {
        #[arg(short, long)]
        list: bool,
    },
    Host {
        #[arg(short, long)]
        list: bool,
    },
    Record {
        #[arg(short = 'd', long = "duration")]
        duration: u64,

        #[arg(short = 'i', long)]
        device_index: Option<usize>,

        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output_file: PathBuf,
    },
    Transcribe {
        #[arg(short = 'm', long = "model")]
        model: PathBuf,

        #[arg(short = 'i', long, value_name = "INPUT_FILE")]
        input_file: PathBuf,

    },
    Live {
        #[arg(short = 'i', long)]
        device_index: Option<usize>,

        #[arg(short = 'm', long = "model")]
        model: PathBuf,
    }

}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();


    match &cli.command {
        Some(Commands::Device { list }) => {
            if *list {
                for device in list_input_devices(cpal::default_host().id()).into_iter().enumerate() {
                    println!("{:?}: {:?}", device.0, device.1.name()?);
                }
            }
            Ok(())
        },
        Some(Commands::Host { list }) => {
            if *list {
                for host in cpal::available_hosts().into_iter().enumerate() {
                    println!("{:?}: {:?}", host.0, host.1);
                }
            }
            Ok(())
        },
        Some(Commands::Record { duration, device_index, output_file }) => {
            let device = match device_index {
                Some(device_index) => {
                    get_input_device(Some(*device_index), Some(cpal::default_host().id()))
                }
                None => get_input_device(None, None),
            };

            println!("Recording using input device {:?}", &device.name());

            let config: cpal::SupportedStreamConfig =
                cpal::SupportedStreamConfig::new(1, cpal::SampleRate(16000),
                                                cpal::SupportedBufferSize::Range { min: 256, max: 512 },
                                                cpal::SampleFormat::F32);

            let wav_spec = hound::WavSpec {
                channels: config.channels() as _,
                sample_rate: config.sample_rate().0 as _,
                bits_per_sample: (config.sample_format().sample_size() * 8) as _,
                sample_format: if config.sample_format().is_float() {
                    hound::SampleFormat::Float
                } else {
                    hound::SampleFormat::Int
                },
            };

            let writer = Arc::new(Mutex::new(Some(hound::WavWriter::create(
                output_file, wav_spec,
            )?)));

            let detatched_writer = writer.clone();

            let stream = initialize_write_stream(device, detatched_writer, config);
            stream.as_ref().unwrap().play()?;

            std::thread::sleep(std::time::Duration::from_secs(*duration));
            drop(stream);
            writer.lock().unwrap().take().unwrap().finalize()?;
            let path: String = output_file.to_string_lossy().into_owned();
            println!("Recording {} complete.", path);

            Ok(())
        },
        Some(Commands::Transcribe { model, input_file }) => {
            let model_path = model.as_os_str();
            let context = WhisperContext::new_with_params(&model_path.to_str().unwrap(), WhisperContextParameters::default()).expect("Failed to load model.");

            let mut state = context.create_state().expect("Failed to create state.");

            let  reader = hound::WavReader::open(input_file)?;

            println!("Input file contains {} samples.", reader.len());
            let samples : Vec<f32> = reader.into_samples::<f32>()
                .map(|s| s.unwrap() as f32)
                .collect();

            let chunk_size = 16000*10;
            let mut chunks: Vec<Vec<f32>> = vec![vec![0.0; chunk_size]; samples.len() / chunk_size + 1];
            for (i, sample) in samples.iter().enumerate() {
                chunks[i / chunk_size][i % chunk_size] = *sample;
            }
                

            println!("Using a buffer size of {} samples.", chunk_size);
            for chunk in chunks {
                let params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                state.full(params, &chunk[..]).expect("Failed to run model.");

                let n_segments = state.full_n_segments().expect("Failed to get number of segments");

                for i in 0..n_segments {
                    println!("{}", state.full_get_segment_text(i).expect("Failed to get text."));
                }
            }

            Ok(())
        },

        Some(Commands::Live { device_index, model }) => {
            let device = match device_index {
                Some(device_index) => {
                    get_input_device(Some(*device_index), Some(cpal::default_host().id()))
                }
                None => get_input_device(None, None),
            };

            println!("Recording using input device {:?}", &device.name());

            let config: cpal::SupportedStreamConfig =
                cpal::SupportedStreamConfig::new(1, cpal::SampleRate(16000),
                                                cpal::SupportedBufferSize::Range { min: 256, max: 512 },
                                                cpal::SampleFormat::F32);

            let buffer = Arc::new(Mutex::new(Buffer::new(model.to_path_buf(), 3 * 16000)));

            let stream = initialize_buffered_stream(device, buffer, config);
            stream.as_ref().unwrap().play()?;

            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        },
        None => {
            Ok(())
        }
    }
}
