#![no_std]

use {
    defmt_rtt as _,
    embassy_rp::{
        adc::{self},
        bind_interrupts,
        flash::{Blocking, Flash},
        peripherals::{FLASH, USB,
        },
        usb,
    },
    panic_probe as _,
};
use embassy_time as _;

bind_interrupts!(pub struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

/// Helper to get unique ID from flash
pub fn get_unique_id(flash: &mut FLASH) -> Option<u64> {
    let mut flash: Flash<'_, FLASH, Blocking, { 16 * 1024 * 1024 }> = Flash::new_blocking(flash);

    // TODO: For different flash chips, we want to handle things
    // differently based on their jedec? That being said: I control
    // the hardware for this project, and our flash supports unique ID,
    // so oh well.
    //
    // let jedec = flash.blocking_jedec_id().unwrap();

    let mut id = [0u8; core::mem::size_of::<u64>()];
    flash.blocking_unique_id(&mut id).unwrap();
    Some(u64::from_be_bytes(id))
}