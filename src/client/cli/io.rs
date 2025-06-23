use super::client::StreamPart;
use anyhow::Result;
use async_stream::stream;
use futures::stream::{Stream, StreamExt};
use rand::prelude::*;
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{Duration, sleep};

const PROMPT: &str = ">>> ";

pub async fn read_user_input() -> Result<String> {
    let mut output = tokio::io::stdout();
    let input = tokio::io::stdin();
    let mut reader = BufReader::new(input);
    let mut buffer = String::new();
    output.write_all(PROMPT.as_bytes()).await?;
    output.flush().await?;
    reader.read_line(&mut buffer).await?;
    Ok(buffer)
}

pub async fn stdout_stream(
    mut stream: Pin<Box<dyn Stream<Item = Result<StreamPart>>>>,
) -> Result<Vec<StreamPart>> {
    let mut output = tokio::io::stdout();
    let mut parts = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(response) => {
                if let StreamPart::Content(ref text) = response {
                    output.write_all(text.as_bytes()).await?
                }
                parts.push(response);
            }
            Err(error) => output.write_all(format!("{}", error).as_bytes()).await?,
        }
        output.flush().await?;
    }
    Ok(parts)
}

#[allow(unused)]
pub fn dummy_ai_response() -> Pin<Box<dyn Stream<Item = String>>> {
    let stream = stream! {
    let mut rng = rand::rng();
    let chunks = vec![
        "thi", "s c", "ould", " be", " a real", " ai r", "esp", "ons", "e, but", " it's n", "ot",
    ];
        for chunk in chunks {
            let millis = (rng.random::<f64>() * 250.0).floor() as u64;
            let duration = Duration::from_millis(millis);
            sleep(duration).await;
            yield chunk.to_string();
        }
    };
    Box::pin(stream)
}
