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

endpoint!(MeasurementEndpoint, (), Measurement, "measurement");
