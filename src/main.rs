use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use hush::sound_capture::device::{get_device, list_input_devices};

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
    Record { #[arg(short, long)]
        duration: u64,

        #[arg(short = 'i', long)]
        device_index: Option<usize>,

        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output_file: PathBuf,
    },
}

fn write_input_data<T, U>(
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

fn initialize_stream(device: cpal::Device, writer: Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>, config: cpal::SupportedStreamConfig) -> Result<cpal::Stream, anyhow::Error> {

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
                    get_device(Some(*device_index), Some(cpal::default_host().id()))
                }
                None => get_device(None, None),
            };

            println!("Recording using input device {:?}", &device.name());

            let config: cpal::SupportedStreamConfig = device
                .default_input_config()
                .expect("Failed to get default input config");

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

            let stream = initialize_stream(device, detatched_writer, config);
            stream.as_ref().unwrap().play()?;

            std::thread::sleep(std::time::Duration::from_secs(*duration));
            drop(stream);
            writer.lock().unwrap().take().unwrap().finalize()?;
            let path: String = output_file.to_string_lossy().into_owned();
            println!("Recording {} complete.", path);

            Ok(())
        },
        None => {
            Ok(())
        }
    }
}
