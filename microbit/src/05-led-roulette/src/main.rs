#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
use microbit::display::blocking::Display;
use microbit::hal::timer::Timer;
use microbit::Board;
use panic_rtt_target as _;
use rtt_target::rtt_init_print;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let mut display = Display::new(board.display_pins);

    let (mut row, mut col) = (0, 0);

    let mut leds = [[0; 5]; 5];

    loop {
        leds[row][col] = 0;
        (row, col) = compute_next_row_and_col(row, col);
        leds[row][col] = 1;
        display.show(&mut timer, leds, 300);
    }
}

fn compute_next_row_and_col(row: usize, col: usize) -> (usize, usize) {
    assert!(row < 5 && col < 5);

    match (row, col) {
        (0, col) if col < 4 => (0, col + 1),
        (row, 4) if row < 4 => (row + 1, 4),
        (4, col) if col > 0 => (4, col - 1),
        (row, 0) if row > 0 => (row - 1, 0),
        _ => panic!("Already passed the assertion, how did we get here???"),
    }
}
