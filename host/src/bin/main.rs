use std::time::Duration;

use host::client::DeviceClient;
use tokio::time::interval;

#[tokio::main]
pub async fn main() {
    println!("Connecting...");
    let client = DeviceClient::new();
    println!("Connected!");

    let mut sub = client.client.subscribe::<icd::MeasurementTopic>(8).await.unwrap();
    tokio::spawn(async move {
        loop {
            let msg = sub.recv().await.unwrap();
            println!("Got measurement: {:?}", msg);
        }
    });

    // Begin repl...
    loop {
        print!("> ");
        let line = host::read_line().await;
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["start"] => {
                client.start_measuring(1000).await.unwrap();
                println!("Started measuring!")
            }
            ["stop"] => {
                client.stop_measuring().await.unwrap();
                println!("Stopped measuring!")
            }
            _ => {
                println!("Unknown command");
            }
        }
    }
}

async fn measurement_listener() {

}
