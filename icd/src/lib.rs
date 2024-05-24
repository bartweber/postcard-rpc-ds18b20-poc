#![no_std]
// You'll need the unused imports soon!
#![allow(unused_imports)]

use postcard::experimental::schema::Schema;
use postcard_rpc::{endpoint, topic};
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, Schema)]
pub struct Measurement {
    pub timestamp: u64,
    pub temp01: u32,
    pub temp02: u32,
}

endpoint!(PingEndpoint, (), Measurement, "ping");
