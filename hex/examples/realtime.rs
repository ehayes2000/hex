use hex::client::voice::client::VoiceClient;

#[tokio::main]
async fn main() {
    let client = VoiceClient::new();
    client.chat().await;
}
