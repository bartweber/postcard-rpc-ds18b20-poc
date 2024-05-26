use std::convert::Infallible;
use postcard_rpc::{
    host_client::{HostClient, HostErr},
    standard_icd::{WireError, ERROR_PATH},
};
use icd::{Measurement, MeasurementEndpoint};

pub struct DeviceClient {
    pub client: HostClient<WireError>,
}

#[derive(Debug)]
pub enum Error<E> {
    Comms(HostErr<WireError>),
    Endpoint(E),
}

impl<E> From<HostErr<WireError>> for Error<E> {
    fn from(value: HostErr<WireError>) -> Self {
        Self::Comms(value)
    }
}

trait FlattenErr {
    type Good;
    type Bad;
    fn flatten(self) -> Result<Self::Good, Error<Self::Bad>>;
}

impl<T, E> FlattenErr for Result<T, E> {
    type Good = T;
    type Bad = E;
    fn flatten(self) -> Result<Self::Good, Error<Self::Bad>> {
        self.map_err(Error::Endpoint)
    }
}

// ---

impl DeviceClient {
    pub fn new() -> Self {
        let client =
            HostClient::new_raw_nusb(|d| d.product_string() == Some("measuring-device"), ERROR_PATH, 8);
        Self { client }
    }

    pub async fn measure(&self, _id: u32) -> Result<Measurement, Error<Infallible>> {
        let val = self.client.send_resp::<MeasurementEndpoint>(&()).await?;
        Ok(val)
    }
}

impl Default for DeviceClient {
    fn default() -> Self {
        Self::new()
    }
}
