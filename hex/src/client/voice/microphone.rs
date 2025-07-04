use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub fn default_microphone_stream() {
    let host = cpal::default_host();
    let mic = host
        .default_input_device()
        .expect("no input device available");

    let speaker = host
        .default_output_device()
        .expect("no output device available");

    let mic_config = mic
        .default_input_config()
        .expect("failed to get input config");

    let speaker_config = speaker
        .default_output_config()
        .expect("failed to get output config");
}
