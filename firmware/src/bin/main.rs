#![no_std]
#![no_main]

use defmt::info;
use ds18b20::{Ds18b20, Resolution};
use embassy_executor::Spawner;
use embassy_rp::{
    peripherals::USB,
    usb::{self, Driver, Endpoint, Out},
};
use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::I2C1;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_time::Delay;
use embassy_usb::UsbDevice;
use one_wire_ds2482::OneWireDS2482;
use one_wire_hal::OneWire;
use postcard_rpc::{
    define_dispatch,
    target_server::{buffers::AllBuffers, configure_usb, example_config, rpc_dispatch},
    WireHeader,
};
use static_cell::ConstStaticCell;

use firmware::{get_unique_id, Irqs};
use icd::{Measurement, MeasurementEndpoint};
use {defmt_rtt as _, panic_probe as _};


static ALL_BUFFERS: ConstStaticCell<AllBuffers<256, 256, 256>> =
    ConstStaticCell::new(AllBuffers::new());

pub struct Context {
    pub one_wire: OneWireDS2482<I2c<'static, I2C1, Blocking>>,
    pub temp_sensor_1: Ds18b20,
    pub delay: Delay,
}

define_dispatch! {
    dispatcher: Dispatcher<
        Mutex = ThreadModeRawMutex,
        Driver = usb::Driver<'static, USB>,
        Context = Context
    >;
    MeasurementEndpoint => async measurement_handler,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // SYSTEM INIT
    info!("Start");

    let mut p = embassy_rp::init(Default::default());
    let unique_id = get_unique_id(&mut p.FLASH).unwrap();
    info!("id: {=u64:016X}", unique_id);

    // DS18B20 INIT
    let mut delay = Delay;
    let sda = p.PIN_2;
    let scl = p.PIN_3;
    let i2c = embassy_rp::i2c::I2c::new_blocking(p.I2C1, scl, sda, embassy_rp::i2c::Config::default());
    let mut ds2482: OneWireDS2482<I2c<I2C1, Blocking>> = OneWireDS2482::new(i2c, 0x18);
    ds2482.ds2482_device_reset().unwrap();
    ds2482.ds2482_write_config(0b0001).unwrap();
    let mut one_wire = ds2482;
    let addr_1 = one_wire.devices(&mut delay).next().unwrap().unwrap();
    info!("found device on address: {:?}", addr_1.0);
    let temp_sensor_1 = Ds18b20::new(addr_1).unwrap();


    // USB/RPC INIT
    let driver = usb::Driver::new(p.USB, Irqs);
    let mut config = example_config();
    config.manufacturer = Some("Bartomatic");
    config.product = Some("measuring-device");
    let buffers = ALL_BUFFERS.take();
    let (device, ep_in, ep_out) = configure_usb(driver, &mut buffers.usb_device, config);
    let context = Context {
        one_wire,
        temp_sensor_1,
        delay,
    };
    let dispatch = Dispatcher::new(&mut buffers.tx_buf, ep_in, context);

    spawner.must_spawn(dispatch_task(ep_out, dispatch, &mut buffers.rx_buf));
    spawner.must_spawn(usb_task(device));
}

/// This actually runs the dispatcher
#[embassy_executor::task]
async fn dispatch_task(
    ep_out: Endpoint<'static, USB, Out>,
    dispatch: Dispatcher,
    rx_buf: &'static mut [u8],
) {
    rpc_dispatch(ep_out, dispatch, rx_buf).await;
}

/// This handles the low level USB management
#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) {
    usb.run().await;
}

async fn measurement_handler(context: &mut Context, header: WireHeader, _rqst: ()) -> Measurement {
    let delay = &mut context.delay;
    let temp_sensor_1 = &mut context.temp_sensor_1;
    let one_wire = &mut context.one_wire;
    ds18b20::start_simultaneous_temp_measurement(one_wire, delay).unwrap();
    Resolution::Bits12.delay_for_measurement_time(delay);
    let data_1 = temp_sensor_1.read_data(one_wire, delay).unwrap();
    let temp01 = data_1.temperature;

    info!("ping: seq - {=u32}", header.seq_no);
    info!("temp01: {=f32}", temp01);
    Measurement {
        temp01,
    }
}
