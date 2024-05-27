#![no_std]
#![no_main]

use core::sync::atomic::Ordering;

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
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_time::{Delay, Duration, Ticker};
use embassy_usb::UsbDevice;
use one_wire_ds2482::OneWireDS2482;
use one_wire_hal::OneWire;
use portable_atomic::AtomicBool;
use postcard_rpc::{
    define_dispatch,
    target_server::{buffers::AllBuffers, configure_usb, example_config, rpc_dispatch},
    WireHeader,
};
use postcard_rpc::target_server::sender::Sender;
use postcard_rpc::target_server::SpawnContext;
use static_cell::{ConstStaticCell, StaticCell};

use {defmt_rtt as _, panic_probe as _};
use firmware::{get_unique_id, Irqs};
use icd::{Measurement, MeasurementTopic, StartMeasuring, StartMeasuringEndpoint, StopMeasuringEndpoint};

pub type OWire = OneWireDS2482<I2c<'static, I2C1, Blocking>>;

static OWIRE: StaticCell<Mutex<ThreadModeRawMutex, OWire>> = StaticCell::new();
static DB18B20: StaticCell<Mutex<ThreadModeRawMutex, Ds18b20>> = StaticCell::new();

static ALL_BUFFERS: ConstStaticCell<AllBuffers<256, 256, 256>> =
    ConstStaticCell::new(AllBuffers::new());

pub struct Context {
    pub one_wire: &'static Mutex<ThreadModeRawMutex, OWire>,
    pub temp_sensor_1: &'static Mutex<ThreadModeRawMutex, Ds18b20>,
    pub delay: Delay,
}

pub struct SpawnCtx {
    pub one_wire: &'static Mutex<ThreadModeRawMutex, OWire>,
    pub temp_sensor_1: &'static Mutex<ThreadModeRawMutex, Ds18b20>,
}

impl SpawnContext for Context {
    type SpawnCtxt = SpawnCtx;

    fn spawn_ctxt(&mut self) -> Self::SpawnCtxt {
        SpawnCtx {
            one_wire: self.one_wire,
            temp_sensor_1: self.temp_sensor_1,
        }
    }
}

define_dispatch! {
    dispatcher: Dispatcher<
        Mutex = ThreadModeRawMutex,
        Driver = Driver<'static, USB>,
        Context = Context
    >;
    StartMeasuringEndpoint => spawn start_measuring_handler,
    StopMeasuringEndpoint => blocking stop_measuring_handler,
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
    let owire_ref = OWIRE.init(Mutex::new(one_wire));
    let temp_sensor_1_ref = DB18B20.init(Mutex::new(temp_sensor_1));


    // USB/RPC INIT
    let driver = usb::Driver::new(p.USB, Irqs);
    let mut config = example_config();
    config.manufacturer = Some("Bartomatic");
    config.product = Some("measuring-device");
    let buffers = ALL_BUFFERS.take();
    let (device, ep_in, ep_out) = configure_usb(driver, &mut buffers.usb_device, config);
    let context = Context {
        one_wire: owire_ref,
        temp_sensor_1: temp_sensor_1_ref,
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

static STOP: AtomicBool = AtomicBool::new(false);

#[embassy_executor::task]
async fn start_measuring_handler(
    context: SpawnCtx,
    header: WireHeader,
    rqst: StartMeasuring,
    sender: Sender<ThreadModeRawMutex, Driver<'static, USB>>,
) {
    let mut one_wire = context.one_wire.lock().await;
    let temp_sensor_1: MutexGuard<ThreadModeRawMutex, Ds18b20> = context.temp_sensor_1.lock().await;
    if sender
        .reply::<StartMeasuringEndpoint>(header.seq_no, &())
        .await
        .is_err()
    {
        defmt::error!("Failed to reply, stopping measuring");
        return;
    }

    let mut ticker = Ticker::every(Duration::from_millis(rqst.interval_ms.into()));
    let mut seq = 0;
    while !STOP.load(Ordering::Acquire) {
        ticker.next().await;
        ds18b20::start_simultaneous_temp_measurement(&mut *one_wire, &mut Delay).unwrap();
        Resolution::Bits12.delay_for_measurement_time(&mut Delay);
        let data_1 = temp_sensor_1.read_data(&mut *one_wire, &mut Delay).unwrap();
        let temp01 = data_1.temperature;
        info!("temp01: {=f32}", temp01);
        let msg = Measurement {
            temp01,
        };
        if sender.publish::<MeasurementTopic>(seq, &msg).await.is_err() {
            defmt::error!("Failed to publish, stopping measuring");
            break;
        }
        seq = seq.wrapping_add(1);
    }

    info!("Stopping!");
    STOP.store(false, Ordering::Release);
}

fn stop_measuring_handler(_context: &mut Context, header: WireHeader, _rqst: ()) -> bool {
    info!("accel_stop: seq - {=u32}", header.seq_no);
    STOP.store(true, Ordering::Release);
    true
}