use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};

const LATENCY_MS: f32 = 150.0;

fn main() {
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
    let latency_frames = (LATENCY_MS / 1_000.0) * config.sample_rate.0 as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;
    let ring = HeapRb::<f32>::new(latency_samples * 2);

    let (mut producer, mut consumer) = ring.split();

    // Fill the samples with 0.0 equal to the length of the delay.
    for _ in 0..latency_samples {
        // The ring buffer has twice as much space as necessary to add latency here,
        // so this should never fail
        producer.try_push(0.0).unwrap();
    }

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let mut output_fell_behind = false;
        for &sample in data {
            if producer.try_push(sample).is_err() {
                output_fell_behind = true;
            }
        }
        if output_fell_behind {
            eprintln!("output stream fell behind: try increasing latency");
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut input_fell_behind = false;
        for sample in data {
            *sample = match consumer.try_pop() {
                Some(s) => s,
                None => {
                    input_fell_behind = true;
                    0.0
                }
            };
        }
        if input_fell_behind {
            eprintln!("input stream fell behind: try increasing latency");
        }
    };

    let input_stream = mic
        .build_input_stream(&config, input_data_fn, err_fn, None)
        .expect("failed to create stream");
    let output_stream = speaker
        .build_output_stream(&config, output_data_fn, err_fn, None)
        .expect("failed to create stream");

    println!("Successfully built streams.");

    input_stream.play().expect("failed to stream input stream");
    output_stream.play().expect("failed to stream output");

    // Run for 3 seconds before closing.
    println!("Playing for 5 seconds... ");
    std::thread::sleep(std::time::Duration::from_secs(5));
    drop(input_stream);
    drop(output_stream);
    println!("Done!");
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
