use async_openai::config::Config;
use async_openai::types::realtime::{
    ConversationItemCreateEvent, Item, ResponseCreateEvent, ServerEvent, SessionUpdateEvent,
};
use futures_util::{StreamExt, future, pin_mut};
use secrecy::ExposeSecret;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;

use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::Request, protocol::Message},
};

use base64::{Engine as _, engine::general_purpose};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};

const ENDPOINT: &str = "realtime";
const DEFAULT_MODEL: &str = "gpt-4o-realtime-preview";

pub struct Realtime<C: Config> {
    config: C,
    model: &'static str,
    audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    input_audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    _audio_stream: Option<Stream>,
    _input_stream: Option<Stream>,
}

impl<C: Config> Realtime<C> {
    pub fn new(config: C) -> Self {
        let audio_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let input_audio_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let audio_stream = Self::setup_audio_stream(audio_buffer.clone()).ok();
        let input_stream = Self::setup_input_stream(input_audio_buffer.clone()).ok();

        Self {
            config,
            model: DEFAULT_MODEL,
            audio_buffer,
            input_audio_buffer,
            _audio_stream: audio_stream,
            _input_stream: input_stream,
        }
    }

    fn setup_audio_stream(
        audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;

        let config = StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(24000),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                let mut buffer = audio_buffer.lock().unwrap();
                for sample in data.iter_mut() {
                    *sample = buffer.pop_front().unwrap_or(0);
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        Ok(stream)
    }

    fn setup_input_stream(
        input_buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config = StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(24000),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let mut buffer = input_buffer.lock().unwrap();
                for &sample in data.iter() {
                    buffer.push_back(sample);
                }
            },
            |err| eprintln!("Input stream error: {}", err),
            None,
        )?;

        stream.play()?;
        Ok(stream)
    }

    pub async fn message(&self) {
        let (tx, rx) = futures_channel::mpsc::unbounded();
        let (audio_tx, audio_rx) = futures_channel::mpsc::unbounded();
        
        tokio::spawn(read_stdin(tx));
        tokio::spawn(read_audio_input(audio_tx, self.input_audio_buffer.clone()));
        
        let request = self.request();
        println!("{:#?}", request);
        let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");

        // output everything to stderr, for rest of the program stdin is used to send items of type "input_text"
        eprintln!("WebSocket handshake complete");

        let (write, read) = ws_stream.split();

        let combined_rx = futures_util::stream::select(
            rx.map(|msg| msg),
            audio_rx.map(|msg| msg)
        );
        
        let stdin_to_ws = combined_rx.map(Ok).forward(write);

        let audio_buffer = self.audio_buffer.clone();
        let ws_to_stdout = {
            read.for_each(move |message| {
                let audio_buffer = audio_buffer.clone();
                async move {
                    let message = message.unwrap();

                    match message {
                        Message::Text(_) => {
                            let data = message.clone().into_data();
                            let server_event: Result<ServerEvent, serde_json::Error> =
                                serde_json::from_slice(&data);
                            match server_event {
                                Ok(server_event) => {
                                    let value = serde_json::to_value(&server_event).unwrap();
                                    let event_type = value["type"].clone();

                                    eprint!("{:32} | ", event_type.as_str().unwrap());

                                    match server_event {
                                        ServerEvent::ResponseOutputItemDone(event) => {
                                            event.item.content.unwrap_or(vec![]).iter().for_each(
                                                |content| {
                                                    if let Some(ref transcript) = content.transcript
                                                    {
                                                        eprintln!(
                                                            "[{:?}]: {}",
                                                            event.item.role,
                                                            transcript.trim(),
                                                        );
                                                    }
                                                },
                                            );
                                        }
                                        ServerEvent::ResponseAudioTranscriptDelta(event) => {
                                            eprint!("{}", event.delta.trim());
                                        }
                                        ServerEvent::ResponseAudioDelta(event) => {
                                            if let Ok(audio_data) =
                                                general_purpose::STANDARD.decode(&event.delta)
                                            {
                                                let mut buffer = audio_buffer.lock().unwrap();
                                                // Convert bytes to i16 samples (PCM16 format)
                                                for chunk in audio_data.chunks_exact(2) {
                                                    let sample =
                                                        i16::from_le_bytes([chunk[0], chunk[1]]);
                                                    buffer.push_back(sample);
                                                }
                                            }
                                        }
                                        ServerEvent::Error(e) => {
                                            eprint!("{e:?}");
                                        }
                                        _ => {}
                                    }
                                }
                                Err(error) => {
                                    eprintln!("failed to deserialize: {error:?}");
                                    eprintln!("{message:?}");
                                }
                            }
                        }
                        Message::Binary(_) => eprintln!("Binary"),
                        Message::Frame(_) => eprintln!("Frame"),
                        Message::Ping(_) => eprintln!("Ping"),
                        Message::Pong(_) => eprintln!("Pong"),
                        Message::Close(_) => {
                            eprintln!("Close");
                            return;
                        }
                    }

                    // after every message add newline
                    eprint!("\n");
                }
            })
        };
        pin_mut!(stdin_to_ws, ws_to_stdout);
        future::select(stdin_to_ws, ws_to_stdout).await;
    }

    fn request(&self) -> Request<()> {
        let mut base_url = url::Url::parse(self.config.api_base()).expect("valid url");
        let _ = base_url.set_scheme("wss");

        let mut request = format!("{}/{}?model={}", base_url, ENDPOINT, self.model)
            .into_client_request()
            .expect("realtime request");

        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", self.config.api_key().expose_secret())
                .parse()
                .unwrap(),
        );
        request
            .headers_mut()
            .insert("OpenAI-Beta", "realtime=v1".parse().unwrap());
        request
    }
}

async fn read_stdin(tx: futures_channel::mpsc::UnboundedSender<Message>) {
    let mut stdin = tokio::io::stdin();
    let set_audio = serde_json::json! ({
      "type": "session.update",
      "session": {
        "modalities": ["text", "audio"],
        "voice": "alloy",
        "input_audio_format": "pcm16",
        "output_audio_format": "pcm16",
      }
    });

    // Create event of type "conversation.item.create"
    let event = serde_json::from_value::<SessionUpdateEvent>(set_audio).expect("audio update");
    // Create WebSocket message from client event
    let message: Message = event.into();
    // send WebSocket message containing event of type "conversation.item.create" to server
    tx.unbounded_send(message).unwrap();

    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };
        buf.truncate(n);

        let text = String::from_utf8_lossy(&buf).into_owned();

        if text.trim() == "quit" {
            tx.close_channel();
            return;
        }

        // Create item from json representation
        let item = Item::try_from(serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": String::from_utf8_lossy(&buf).into_owned()
                }
            ]
        }))
        .unwrap();

        // Create event of type "conversation.item.create"
        let event: ConversationItemCreateEvent = item.into();
        // Create WebSocket message from client event
        let message: Message = event.into();
        // send WebSocket message containing event of type "conversation.item.create" to server
        tx.unbounded_send(message).unwrap();
        // send WebSocket message containing event of type "response.create" to server
        tx.unbounded_send(ResponseCreateEvent::default().into())
            .unwrap();
    }
}

async fn read_audio_input(
    tx: futures_channel::mpsc::UnboundedSender<Message>,
    input_buffer: Arc<Mutex<VecDeque<i16>>>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    const SILENCE_THRESHOLD: i16 = 1000; // Adjust this threshold as needed
    const SILENCE_DURATION_MS: u64 = 500; // Commit after 500ms of silence
    
    let mut last_voice_time = std::time::Instant::now();
    let mut has_sent_audio = false;
    
    loop {
        interval.tick().await;
        
        let mut samples = Vec::new();
        {
            let mut buffer = input_buffer.lock().unwrap();
            while let Some(sample) = buffer.pop_front() {
                samples.push(sample);
            }
        }
        
        if samples.is_empty() {
            // Check if we should commit due to silence timeout
            if has_sent_audio && last_voice_time.elapsed().as_millis() > SILENCE_DURATION_MS as u128 {
                let commit_event = serde_json::json!({
                    "type": "input_audio_buffer.commit"
                });
                
                let message = Message::Text(commit_event.to_string().into());
                if tx.unbounded_send(message).is_err() {
                    break;
                }
                
                has_sent_audio = false;
                eprint!(" [committed] ");
            }
            continue;
        }
        
        // Simple voice activity detection - check if any samples exceed threshold
        let has_voice = samples.iter().any(|&sample| sample.abs() > SILENCE_THRESHOLD);
        
        if has_voice {
            last_voice_time = std::time::Instant::now();
            
            // Convert i16 samples to bytes
            let mut audio_bytes = Vec::new();
            for sample in samples {
                audio_bytes.extend_from_slice(&sample.to_le_bytes());
            }
            
            // Base64 encode the audio data
            let base64_audio = general_purpose::STANDARD.encode(&audio_bytes);
            
            // Create input_audio_buffer.append event
            let event = serde_json::json!({
                "type": "input_audio_buffer.append",
                "audio": base64_audio
            });
            
            let message = Message::Text(event.to_string().into());
            if tx.unbounded_send(message).is_err() {
                break;
            }
            
            has_sent_audio = true;
            eprint!("🎤");
        }
    }
}
