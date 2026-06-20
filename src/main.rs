use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, StreamTrait};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

use hush::api::create_router;
use hush::api::rest::AppState;
use hush::config::ServerConfig;
use hush::device::{get_input_device, list_input_devices};
use hush::inference::WhisperModel;
use hush::service::session::ClientConnection;
use hush::service::transcription::TranscriptionService;
use hush::utils::{Buffer, initialize_buffered_stream, initialize_write_stream};

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

    #[arg(short = 'l', long = "language")]
    language: String,
  },
  Serve(ServerConfig),
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::from_default_env().add_directive("hush=info".parse()?),
    )
    .init();

  let cli = Cli::parse();

  match &cli.command {
    Some(Commands::Device { list }) => {
      if *list {
        for device in list_input_devices(cpal::default_host().id())
          .into_iter()
          .enumerate()
        {
          println!("{:?}: {:?}", device.0, device.1.name()?);
        }
      }
      Ok(())
    }
    Some(Commands::Host { list }) => {
      if *list {
        for host in cpal::available_hosts().into_iter().enumerate() {
          println!("{:?}: {:?}", host.0, host.1);
        }
      }
      Ok(())
    }
    Some(Commands::Record {
      duration,
      device_index,
      output_file,
    }) => {
      let device = match device_index {
        Some(device_index) => {
          get_input_device(Some(*device_index), Some(cpal::default_host().id()))
        }
        None => get_input_device(None, None),
      };

      println!("Recording using input device {:?}", &device.name());

      let config = cpal::SupportedStreamConfig::new(
        1,
        cpal::SampleRate(16000),
        cpal::SupportedBufferSize::Range { min: 256, max: 512 },
        cpal::SampleFormat::F32,
      );

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
        output_file,
        wav_spec,
      )?)));

      let detached_writer = writer.clone();

      let stream = initialize_write_stream(device, detached_writer, config);
      stream.as_ref().unwrap().play()?;

      std::thread::sleep(std::time::Duration::from_secs(*duration));
      drop(stream);
      writer.lock().unwrap().take().unwrap().finalize()?;
      let path: String = output_file.to_string_lossy().into_owned();
      println!("Recording {} complete.", path);

      Ok(())
    }
    Some(Commands::Transcribe { model, input_file }) => {
      let reader = hound::WavReader::open(input_file)?;
      let spec = reader.spec();

      println!("Input file contains {} samples.", reader.len());
      println!(
        "Sample format: {:?}, {} bits",
        spec.sample_format, spec.bits_per_sample
      );

      let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
          .into_samples::<f32>()
          .collect::<Result<Vec<f32>, _>>()?,
        hound::SampleFormat::Int => {
          let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
          reader
            .into_samples::<i32>()
            .collect::<Result<Vec<i32>, _>>()?
            .into_iter()
            .map(|s| s as f32 / max)
            .collect()
        }
      };

      let mut buffer = Buffer::new(model.to_path_buf(), None, 3 * 16000);

      for sample in samples {
        buffer.push(sample)
      }

      Ok(())
    }

    Some(Commands::Live {
      device_index,
      model,
      language,
    }) => {
      let device = match device_index {
        Some(device_index) => {
          get_input_device(Some(*device_index), Some(cpal::default_host().id()))
        }
        None => get_input_device(None, None),
      };

      println!("Recording using input device {:?}", &device.name());

      let config = cpal::SupportedStreamConfig::new(
        1,
        cpal::SampleRate(16000),
        cpal::SupportedBufferSize::Range { min: 256, max: 512 },
        cpal::SampleFormat::F32,
      );

      let buffer = Arc::new(Mutex::new(Buffer::new(
        model.to_path_buf(),
        Some(language.to_string()),
        3 * 16000,
      )));

      let stream = initialize_buffered_stream(device, buffer, config);
      stream.as_ref().unwrap().play()?;

      loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
      }
    }
    Some(Commands::Serve(config)) => {
      info!("Starting Hush server on {}", config.address());
      info!("Model: {}", config.model_path.display());
      info!("Language: {}", config.language);
      if config.mic {
        info!("Mic capture enabled");
      }

      let model = Arc::new(tokio::sync::Mutex::new(WhisperModel::new(
        &config.model_path,
      )?));
      let transcription = Arc::new(TranscriptionService::new(model, config.language.clone()));

      let sessions: Arc<tokio::sync::Mutex<Vec<ClientConnection>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));

      if config.mic {
        let mic_config = config.clone();
        let mic_transcription = transcription.clone();
        let mic_sessions = sessions.clone();

        std::thread::spawn(move || {
          if let Err(e) =
            hush::mic::start_mic_capture(&mic_config, mic_transcription, mic_sessions)
          {
            error!("Mic capture failed: {}", e);
          }
        });
      }

      let state = AppState {
        config: config.clone(),
        transcription,
        sessions,
      };

      let app = create_router(state);

      let listener = tokio::net::TcpListener::bind(&config.address()).await?;
      info!("Server listening on http://{}", config.address());

      axum::serve(listener, app).await?;

      Ok(())
    }
    None => Ok(()),
  }
}
