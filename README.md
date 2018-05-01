Rust cff3000
============

The Rust `cff3000` can be used to work control a GPIO connected
Abus CFF300 remote control. The crate provides methods to lock,
unlock or query the CFA3000 state.

Examples
========

```rust
extern crate cff3000;
use cff3000::CFF3000;
use std::io::{Error,ErrorKind};

fn execute(cmd: &str) -> std::io::Result<()> {
    let cff3000 = try!(CFF3000::new("/dev/gpiochip2", [2,3,4,5]));
    let duration: u8;

    match cmd {
        "lock" => {try!(cff3000.lock()); duration = 10;},
        "unlock" => {try!(cff3000.unlock()); duration = 10;},
        "check" => {try!(cff3000.check()); duration = 8;},
        _ => return Err(Error::new(ErrorKind::Other, "unsupported command")),
    }

    try!(cff3000.show_leds(duration));
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("missing parameter: lock, unlock, check");
        std::process::exit(1)
    }

    match execute(args[1].as_str()) {
        Err(err) => println!("{}", err.to_string()),
        Ok(()) => {},
    }
}
```

License
=======

© 2018 Sebastian Reichel

ISC License

Permission to use, copy, modify, and/or distribute this software for
any purpose with or without fee is hereby granted, provided that the
above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
