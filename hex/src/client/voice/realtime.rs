use async_openai::config::Config;
use async_openai::types::realtime::{
    ConversationItemCreateEvent, Item, ResponseCreateEvent, ServerEvent, SessionUpdateEvent,
};
use futures_util::{StreamExt, future, pin_mut};
use secrecy::ExposeSecret;
use tokio::io::AsyncReadExt;

use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::Request, protocol::Message},
};

const ENDPOINT: &str = "realtime";
const DEFAULT_MODEL: &str = "gpt-4o-realtime-preview-2024-12-17";

pub struct Realtime<C: Config> {
    config: C,
    model: &'static str,
}

impl<C: Config> Realtime<C> {
    pub fn new(mut config: C) -> Self {
        Self {
            config,
            model: DEFAULT_MODEL,
        }
    }

    pub async fn message(&self) {
        let (tx, rx) = futures_channel::mpsc::unbounded();
        tokio::spawn(read_stdin(tx));
        let request = self.request();
        println!("{:#?}", request);
        let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");

        // output everything to stderr, for rest of the program stdin is used to send items of type "input_text"
        eprintln!("WebSocket handshake complete");

        let (write, read) = ws_stream.split();

        let stdin_to_ws = rx.map(Ok).forward(write);

        let ws_to_stdout = {
            read.for_each(|message| async {
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
                                                if let Some(ref transcript) = content.transcript {
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
            })
        };
        pin_mut!(stdin_to_ws, ws_to_stdout);
        future::select(stdin_to_ws, ws_to_stdout).await;
    }

    fn request(&self) -> Request<()> {
        let mut base_url = url::Url::parse(self.config.api_base()).expect("valid url");
        base_url.set_scheme("wss");

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
