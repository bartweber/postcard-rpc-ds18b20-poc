use std::time::Duration;

use host::client::DeviceClient;
use tokio::time::interval;

#[tokio::main]
pub async fn main() {
    let client = DeviceClient::new();
    let mut ticker = interval(Duration::from_millis(250));

    for i in 0..10 {
        ticker.tick().await;
        print!("Measurement #{i}... ");
        let res = client.measure(i).await.unwrap();
        println!("got {:?}!", res);
        // assert_eq!(res, i);
    }
}
