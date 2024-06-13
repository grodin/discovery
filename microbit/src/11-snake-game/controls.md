# Controls

Our protagonist will be controlled by the two buttons on the front of the micro:bit. Button A will turn to the (snake's)
left, and button B will turn to the (snake's) right.

We will use the `microbit::pac::interrupt` macro to handle button presses in a concurrent way. The interrupt will be
generated by the micro:bit's GPIOTE (**G**eneral **P**urpose **I**nput/**O**utput **T**asks and **E**vents) peripheral.

## The `controls` module

Code in this section should be placed in a separate file, `controls.rs`, in our `src` directory.

We will need to keep track of two separate pieces of global mutable state: A reference to the `GPIOTE` peripheral, and a
record of the selected direction to turn next.

```rust
use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use microbit::hal::gpiote::Gpiote;
use crate::game::Turn;

// ...

static GPIO: Mutex<RefCell<Option<Gpiote>>> = Mutex::new(RefCell::new(None));
static TURN: Mutex<RefCell<Turn>> = Mutex::new(RefCell::new(Turn::None));
```

The data is wrapped in a `RefCell` to permit interior mutability. You can learn more about `RefCell` by reading
[its documentation](https://doc.rust-lang.org/std/cell/struct.RefCell.html) and the relevant chapter of [the Rust Book](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html).
The `RefCell` is, in turn, wrapped in a `cortex_m::interrupt::Mutex` to allow safe access.
The Mutex provided by the `cortex_m` crate uses the concept of a [critical section](https://en.wikipedia.org/wiki/Critical_section).
Data in a Mutex can only be accessed from within a function or closure passed to `cortex_m::interrupt:free`, which
ensures that the code in the function or closure cannot itself be interrupted.

First, we will initialise the buttons.

```rust
use cortex_m::interrupt::free;
use microbit::{
    board::Buttons,
    pac::{self, GPIOTE}
};

// ...

/// Initialise the buttons and enable interrupts.
pub(crate) fn init_buttons(board_gpiote: GPIOTE, board_buttons: Buttons) {
    let gpiote = Gpiote::new(board_gpiote);

    let channel0 = gpiote.channel0();
    channel0
        .input_pin(&board_buttons.button_a.degrade())
        .hi_to_lo()
        .enable_interrupt();
    channel0.reset_events();

    let channel1 = gpiote.channel1();
    channel1
        .input_pin(&board_buttons.button_b.degrade())
        .hi_to_lo()
        .enable_interrupt();
    channel1.reset_events();

    free(move |cs| {
        *GPIO.borrow(cs).borrow_mut() = Some(gpiote);

        unsafe {
            pac::NVIC::unmask(pac::Interrupt::GPIOTE);
        }
        pac::NVIC::unpend(pac::Interrupt::GPIOTE);
    });
}
```

The `GPIOTE` peripheral on the nRF52 has 8 "channels", each of which can be connected to a `GPIO` pin and configured to
respond to certain events, including rising edge (transition from low to high signal) and falling edge (high to low
signal). A button is a `GPIO` pin which has high signal when not pressed and low signal otherwise. Therefore, a button
press is a falling edge.

We connect `channel0` to `button_a` and `channel1` to `button_b` and, in each case, tell them to generate events on a
falling edge (`hi_to_lo`). We store a reference to our `GPIOTE` peripheral in the `GPIO` Mutex. We then `unmask` `GPIOTE`
interrupts, allowing them to be propagated by the hardware, and call `unpend` to clear any interrupts with pending
status (which may have been generated prior to the interrupts being unmasked).

Next, we write the code that handles the interrupt. We use the `interrupt` macro provided by `microbit::pac` (in the
case of the v2, it is re-exported from the `nrf52833_hal` crate). We define a function with the same name as the
interrupt we want to handle (you can see them all [here](https://docs.rs/nrf52833-hal/latest/nrf52833_hal/pac/enum.Interrupt.html)) and annotate it with `#[interrupt]`.

```rust
use microbit::pac::interrupt;

// ...

#[interrupt]
fn GPIOTE() {
    free(|cs| {
        if let Some(gpiote) = GPIO.borrow(cs).borrow().as_ref() {
            let a_pressed = gpiote.channel0().is_event_triggered();
            let b_pressed = gpiote.channel1().is_event_triggered();

            let turn = match (a_pressed, b_pressed) {
                (true, false) => Turn::Left,
                (false, true) => Turn::Right,
                _ => Turn::None
            };

            gpiote.channel0().reset_events();
            gpiote.channel1().reset_events();

            *TURN.borrow(cs).borrow_mut() = turn;
        }
    });
}
```

When a `GPIOTE` interrupt is generated, we check each button to see whether it has been pressed. If only button A has been
pressed, we record that the snake should turn to the left. If only button B has been pressed, we record that the snake
should turn to the right. In any other case, we record that the snake should not make any turn. The relevant turn is
stored in the `TURN` Mutex. All of this happens within a `free` block, to ensure that we cannot be interrupted again
while handling this interrupt.

Finally, we expose a simple function to get the next turn.

```rust
/// Get the next turn (i.e., the turn corresponding to the most recently pressed button).
pub fn get_turn(reset: bool) -> Turn {
    free(|cs| {
        let turn = *TURN.borrow(cs).borrow();
        if reset {
            *TURN.borrow(cs).borrow_mut() = Turn::None
        }
        turn
    })
}
```

This function simply returns the current value of the `TURN` Mutex. It takes a single boolean argument, `reset`. If
`reset` is `true`, the value of `TURN` is reset, i.e., set to `Turn::None`.

## Updating the `main` file

Returning to our `main` function, we need to add a call to `init_buttons` before our main loop, and in the game loop,
replace our placeholder `Turn::None` argument to the `game.step` method with the value returned by `get_turn`.

```rust
#![no_main]
#![no_std]

mod game;
mod control;

use cortex_m_rt::entry;
use microbit::{
    Board,
    hal::{prelude::*, Rng, Timer},
    display::blocking::Display
};
use rtt_target::rtt_init_print;
use panic_rtt_target as _;

use crate::game::{Game, GameStatus};
use crate::control::{init_buttons, get_turn};

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let mut board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let mut rng = Rng::new(board.RNG);
    let mut game = Game::new(rng.random_u32());

    let mut display = Display::new(board.display_pins);

    init_buttons(board.GPIOTE, board.buttons);

    loop {  // Main loop
        loop {  // Game loop
            let image = game.game_matrix(9, 9, 9);
            // The brightness values are meaningless at the moment as we haven't yet
            // implemented a display capable of displaying different brightnesses
            display.show(&mut timer, image, game.step_len_ms());
            match game.status {
                GameStatus::Ongoing => game.step(get_turn(true)),
                _ => {
                    for _ in 0..3 {
                        display.clear();
                        timer.delay_ms(200u32);
                        display.show(&mut timer, image, 200);
                    }
                    display.clear();
                    display.show(&mut timer, game.score_matrix(), 1000);
                    break
                }
            }
        }
        game.reset();
    }
}
```

Now we can control the snake using the micro:bit's buttons!