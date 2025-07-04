use super::io_device::default_io;
use super::realtime::Realtime;
use async_openai::Client;
use async_openai::config::{Config, OpenAIConfig};

use cpal::Device;
use cpal::Stream as AudioStream;

pub struct VoiceClient<C: Config> {
    config: C, // toolset: SyncToolSet<NoContext>,
               // messages: Vec<ChatCompletionRequestMessage>,
               // input_device: Device,
               // output_stream: AudioStream,
}

impl VoiceClient<OpenAIConfig> {
    pub fn new() -> Self {
        let config = OpenAIConfig::new();
        // let (mic, speaker, config) = default_io();
        // let output_stream = L
        Self { config }
    }
}

impl<C: Config> VoiceClient<C> {
    pub async fn chat(&self) {
        Realtime::new(self.config.clone()).message().await
    }
}
