#![no_main]
#![no_std]

use cortex_m_rt::entry;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};
use core::fmt::Write;


use microbit::{
    hal::prelude::*,
    hal::uarte,
    hal::uarte::{Baudrate, Parity},
};

mod serial_setup;
use serial_setup::UartePort;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = microbit::Board::take().unwrap();


    let mut serial = {
        let serial = uarte::Uarte::new(
            board.UARTE0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );
        UartePort::new(serial)
    };


    let mut input_buffer = heapless::Vec::<u8, 32>::new();
    loop {
        input_buffer.clear();
        let mut byte = 0_u8;

        let success = loop {
            byte = nb::block!(serial.read()).unwrap();

            if byte == b'\r' {
                break true;
            }

            if input_buffer.push(byte).is_err() {
                write!(serial, "Error: input buffer is full!\r\n").unwrap();
                break false;
            }
        };
        if success {
            input_buffer.reverse();
            write!(serial, "{}\r\n", core::str::from_utf8(&input_buffer).unwrap()).unwrap();
        }
        nb::block!(serial.flush()).unwrap();
    }
}
