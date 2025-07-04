use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, StreamConfig};

pub fn default_io() -> (Device, Device, StreamConfig) {
    let host = cpal::default_host();
    let mic = host
        .default_input_device()
        .expect("no input device available");

    let speaker = host
        .default_output_device()
        .expect("no output device available");

    let config: StreamConfig = mic
        .default_input_config()
        .expect("failed to get input config")
        .into();
    (mic, speaker, config)
}
