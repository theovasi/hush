# Hush CLI
Hush is a command-line tool for audio recording and transcription using the Whisper speech recognition model.

## Installation
``
cargo install hush
``

## Usage
``
hush [COMMAND] [OPTIONS]
``

### Commands
List all available audio input devices with their indexes for reference.

``
hush device --list
``

### List all available audio hosts on your system.

``
hush host --list
``

### Record audio for the specified duration and save it to a WAV file.

``
hush record --duration <SECONDS> --output-file <OUTPUT_FILE> [--device-index <INDEX>]

Options:

-d, --duration <SECONDS>: Recording duration in seconds
-o, --output-file <OUTPUT_FILE>: Path to save the WAV file
-i, --device-index <INDEX>: Optional index of the input device to use (defaults to system default)
``

### Transcribe an audio file using the specified Whisper model.

``
hush transcribe --model <MODEL_PATH> --input-file <INPUT_FILE>

Options:

-m, --model <MODEL_PATH>: Path to the Whisper model file
-i, --input-file <INPUT_FILE>: Path to the audio file to transcribe
``

### Perform real-time transcription from a microphone using the specified Whisper model.

``
hush live --model <MODEL_PATH> --language <LANGUAGE> [--device-index <INDEX>]

Options:

-m, --model <MODEL_PATH>: Path to the Whisper model file
-l, --language <LANGUAGE>: Language code for transcription
-i, --device-index <INDEX>: Optional index of the input device to use (defaults to system default)
``


Audio recording uses a 16kHz sample rate with mono channel
For live transcription, the language code should match one of the languages supported by the Whisper model

