#![no_std]
// You'll need the unused imports soon!
#![allow(unused_imports)]

use postcard::experimental::schema::Schema;
use postcard_rpc::{endpoint, topic};
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, Schema)]
pub struct Measurement {
    pub temp01: f32,
}

#[derive(Debug, Serialize, Deserialize, Schema, PartialEq)]
pub struct StartMeasuring {
    pub interval_ms: u32,
    pub threshold: f32,
}

endpoint!(StartMeasuringEndpoint, StartMeasuring, (), "measuring/start");
endpoint!(StopMeasuringEndpoint, (), bool, "measuring/stop");
topic!(MeasurementTopic, Measurement, "measuring/data");
