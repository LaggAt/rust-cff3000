// © 2018 Sebastian Reichel
// SPDX-License-Identifier: ISC

#![crate_type = "lib"]
#![crate_name = "cff3000"]

//! The `cff3000` crate provides a highlevel API for a GPIO connected
//! CFF3000 remote control.
//!
//! # Example
//! ```
//! extern crate cff3000;
//! use cff3000::CFF3000;
//! use std::io::{Error,ErrorKind};
//! 
//! fn execute(cmd: &str) -> std::io::Result<()> {
//!     let cff3000 = try!(CFF3000::new("/dev/gpiochip2", [2,3,4,5]));
//!     let duration: u8;
//! 
//!     match cmd {
//!         "lock" => {try!(cff3000.lock()); duration = 10;},
//!         "unlock" => {try!(cff3000.unlock()); duration = 10;},
//!         "check" => {try!(cff3000.check()); duration = 8;},
//!         _ => return Err(Error::new(ErrorKind::Other, "unsupported command")),
//!     }
//! 
//!     try!(cff3000.show_leds(duration));
//!     Ok(())
//! }
//! 
//! fn main() {
//!     let args: Vec<String> = std::env::args().collect();
//! 
//!     if args.len() < 2 {
//!         println!("missing parameter: lock, unlock, check");
//!         std::process::exit(1)
//!     }
//! 
//!     match execute(args[1].as_str()) {
//!         Err(err) => println!("{}", err.to_string()),
//!         Ok(()) => {},
//!     }
//! }
//! ```

extern crate gpiochip as gpio;
use std::io::Write;

pub struct CFF3000 {
    red: gpio::GpioEventHandle,
    green: gpio::GpioEventHandle,
    unlock: gpio::GpioHandle,
    lock: gpio::GpioHandle,
}

#[derive(Debug, Copy, Clone)]
struct Event {
    /// led (0 = red, 1 = green)
    mask: u8,
    /// timestamp (in ms)
    timestamp: u64,
    /// enabled = HIGH, otherwise LOW
    state: u8,
}

#[derive(Debug)]
#[derive(PartialEq)]
pub enum CFF3000State {
    /// The door is locked (green LED on)
    Locked,
    /// The door is unlocked (red LED on)
    Unlocked,
    /// The door state has been changed manually (both LEDs blink synchronously)
    Manual,
    /// The CFA3000 is out of range (both LEDs blink alternating)
    OutOfRange,
}

impl CFF3000 {
    /// Create new CFF3000 device.
    ///
    /// This functions requires permissions to open the gpiochip
    /// device. `chipdev` should be something like "/dev/gpiochip0"
    /// and `gpios` should be an array containing the line offsets
    /// for LED red, LED green, button unlock and button lock (in
    /// this order)
    pub fn new(chipdev: &str, gpios: [u32; 4]) -> std::io::Result<CFF3000> {
        let chip = try!(gpio::GpioChip::new(chipdev));
        let led_red = try!(chip.request_event("led-red", gpios[0], gpio::RequestFlags::INPUT, gpio::EventRequestFlags::BOTH_EDGES));
        let led_green = try!(chip.request_event("led-green", gpios[1], gpio::RequestFlags::INPUT, gpio::EventRequestFlags::BOTH_EDGES));
        let button_unlock = try!(chip.request("button-unlock", gpio::RequestFlags::OUTPUT, gpios[2], 0));
        let button_lock = try!(chip.request("button-lock", gpio::RequestFlags::OUTPUT, gpios[3], 0));
        Ok(CFF3000 {red: led_red, green: led_green, unlock: button_unlock, lock: button_lock})
    }

    fn print_leds(red: bool, green: bool) -> std::io::Result<()> {
        if red && green {
            print!("\r\x1b[31m ● \x1b[32m● \x1b[0m ")
        } else if red {
            print!("\r\x1b[31m ● \x1b[32m◯ \x1b[0m ")
        } else if green {
            print!("\r\x1b[31m ◯ \x1b[32m● \x1b[0m ")
        } else {
            print!("\r\x1b[31m ◯ \x1b[32m◯ \x1b[0m ")
        }
        try!(std::io::stdout().flush());
        Ok(())
    }

    /// Print LED state to stdout for `duration` seconds.
    ///
    /// This will print the current LED state of the CFF3000 using colored
    /// UTF-8 symbols. The output will be refreshed for `duration` seconds
    /// using the rollback character.
    pub fn show_leds(&self, duration: u8) -> std::io::Result<()> {
        let mut r = false;
        let mut g = false;
        let start = std::time::Instant::now();

        try!(CFF3000::print_leds(r, g));

        while start.elapsed().as_secs() < duration as u64 {
            let events = try!(gpio::wait_for_event(&[&self.red, &self.green], 1000));
            if events == 0 {
                continue;
            }

            if events & 0x1 != 0 {
                r = try!(self.red.read()).id == gpio::EventId::RISING_EDGE;
            }
            if events & 0x2 != 0 {
                g = try!(self.green.read()).id == gpio::EventId::RISING_EDGE;
            }

            try!(CFF3000::print_leds(r, g));
        }

        println!("");

        Ok(())
    }

    /// Press and release lock button.
    pub fn lock(&self) -> std::io::Result<()> {
        try!(self.lock.set(1));
        std::thread::sleep(std::time::Duration::from_millis(500));
        try!(self.lock.set(0));
        Ok(())
    }

    /// Press and release unlock button.
    pub fn unlock(&self) -> std::io::Result<()> {
        try!(self.unlock.set(1));
        std::thread::sleep(std::time::Duration::from_millis(500));
        try!(self.unlock.set(0));
        Ok(())
    }

    /// Press and release both buttons to query state.
    pub fn check(&self) -> std::io::Result<()> {
        try!(self.lock.set(1));
        try!(self.unlock.set(1));
        std::thread::sleep(std::time::Duration::from_millis(500));
        try!(self.unlock.set(0));
        try!(self.lock.set(0));
        Ok(())
    }

    /// Query CFA3000 state and interpret the following
    /// LED pattern. This function blocks for 8 seconds
    /// to capture the LED blink pattern.
    pub fn state(&self) -> std::io::Result<CFF3000State> {
        try!(self.check());

        let mut eventlog: std::vec::Vec<Event> = std::vec::Vec::new();
        let start = std::time::Instant::now();

        print!("waiting for led events... ");
        try!(std::io::stdout().flush());

        while start.elapsed().as_secs() < 8 {
            let events = try!(gpio::wait_for_event(&[&self.red, &self.green], 1000));
            if events == 0 {
                continue;
            }

            if events & 0x1 != 0 {
                let event = try!(self.red.read());
                let state: u8 = if event.id == gpio::EventId::RISING_EDGE {1} else {0};
                eventlog.push(Event {mask: 0b01, timestamp: event.timestamp/1000/1000, state: state << 0});
            }
            if events & 0x2 != 0 {
                let event = try!(self.green.read());
                let state: u8 = if event.id == gpio::EventId::RISING_EDGE {1} else {0};
                eventlog.push(Event {mask: 0b10, timestamp: event.timestamp/1000/1000, state: state << 1});
            }
        }

        /* combine events within 50ms */
        let mut simple_eventlog: std::vec::Vec<Event> = std::vec::Vec::new();
        simple_eventlog.push(eventlog[0]);
        for i in 1..eventlog.len() {
            if eventlog[i-1].timestamp > eventlog[i].timestamp - 50 {
                let pos = simple_eventlog.len()-1;
                simple_eventlog[pos].mask |= eventlog[i].mask;
                simple_eventlog[pos].state |= eventlog[i].state & eventlog[i].mask;
                simple_eventlog[pos].state &= eventlog[i].state | !eventlog[i].mask;
            } else {
                simple_eventlog.push(eventlog[i]);
            }
        }

        /* fill up event data for unchanged leds with previous information and use relative timestamps */
        let mut state = 0u8;
        let offset = simple_eventlog[0].timestamp;
        for e in &mut simple_eventlog {
            state &= !e.mask;
            state |= e.state & e.mask;

            if e.mask != 0b11 {
                e.state &= e.mask;
                e.state |= state;
            }

            e.mask = 0b11;
            e.timestamp -= offset;
        }

        /* check for obvious problems */
        if simple_eventlog.len() <= 2 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "did not receive enough LED change events"));
        }
        if simple_eventlog.first().unwrap().state != 0b11 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "first LED changes is invalid"));
        }
        if simple_eventlog.last().unwrap().state != 0b00 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "last LED change is invalid"));
        }

        let result: CFF3000State;
        if simple_eventlog.len() == 3 {
            if simple_eventlog[1].state == 0b10 {
                result = CFF3000State::Locked;
            } else if simple_eventlog[1].state == 0b01 {
                result = CFF3000State::Unlocked;
            } else {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid LED state"));
            }
        } else {
            result = match simple_eventlog[1].state {
                0b00 => CFF3000State::Manual,
                0b01 => CFF3000State::OutOfRange,
                0b10 => CFF3000State::OutOfRange,
                _ => {return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid LED state"))},
            };

            for i in 2..simple_eventlog.len()-1 {
                if result == CFF3000State::Manual {
                    if simple_eventlog[i-1].state & 0b11 != !simple_eventlog[i].state & 0b11 {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid manual error LED substate"));
                    }
                } else {
                    if simple_eventlog[i].state == 0b00 || simple_eventlog[i].state == 0b11 {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid out of range error LED substate"));
                    }
                    if simple_eventlog[i-1].state & 0b11 == !simple_eventlog[i-1].state & 0b11 {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid out of range error LED substate"));
                    }
                }
            }
        }

        Ok(result)
    }
}
