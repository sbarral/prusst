# prusst

[![Build Status](https://travis-ci.org/sbarral/prusst.svg?branch=master)](https://travis-ci.org/sbarral/prusst)
[![Version](http://meritbadge.herokuapp.com/prusst)](https://crates.io/crates/prusst)

A convenient Rust interface to the UIO kernel module for TI Programmable Real-time Unit
coprocessors found among others on the BeagleBone development boards. It provides roughly the same
functionality as the [C prussdrv library](https://github.com/beagleboard/am335x_pru_package)
but with a safer, rustic API that attempts to mitigate risks related to uninitialized or
invalid register states, use of freed memory, memory allocations conflicts etc.


## Documentation

The API documentation lives [here](https://sbarral.github.io/prusst-doc/prusst/).


## Background

PRUs (Programmable Real-time Units) are RISC cores integrated into some TI processors such as
the AM335x that powers the BeagleBone::{White, Black, Green} development boards. They are
what sets the BeagleBone apart from other popular boards such as the Raspberry Pi, allowing
real-time process control without the complexity associated with an external co-processor.

PRUs have direct access to some general purpose I/O as well as indirect access to memory and
peripherals via an interconnect bus. Their predictable single-cycle instruction execution and
absence of pipe-lining or caching is what makes them especially suitable for real-time processing.

Since the PRU assembly language is simple and enables total control of the execution timing,
critical real-time portions are typically programmed directly in assembly for the PRU,
which cooperates with the host processor for the heavy-lifting (pre and post-processing,
communication etc.).

This library provides a relatively simple abstraction over the UIO kernel module which makes
it easy to perform common operations such as executing code on the PRU, transferring data
between the PRU and the host processor or triggering/waiting for system events.


## Design rationale

The design of the library exploits the Rust type system to reduce the risk of shooting onself
in the foot. Its architecture is meant to offer improved ergonomics compared to its C relative,
while operating at a similarly low level of abstraction and providing equivalent functionality.

Data-race safety is warranted by checking that only one `Pruss` instance (a view of the PRU
subsystem) is running at a time. The magic of the Rust borrowing rules will then _statically_
ensure, inter alia:

* the absence of memory aliasing for local and shared PRU RAM, meaning that a previously allocated
RAM segment may not be re-used before the data it contains is released,

* the impossibility to request code execution on a PRU core before the code has actually been
loaded,

* the impossibility to overwrite PRU code that is already loaded and still in use,

* the impossibility to concurrently modify the interrupt mapping.

Type safety also avoids many pitfalls associated with interrupt management. Unlike the C prussdrv
library, system events, host interrupt, events out and channels are all distinct types: they cannot
be misused or inadvertently switched in function calls. A related benefit is that the interrupt
management API is very self-explanatory.

Event handling is one of the few places where prusst requires the user to be more explicit
than the C prussdrv library. Indeed, the `prussdrv_pru_clear_event` function of the C driver
automatically re-enables an event out after clearing the triggering system event, which may wrongly
suggest that the combined clear-enable operation is thread-safe (it isn't). In contrast, prusst
mandates that both `Intc::clear_sysevt` and `Intc::enable_host` be called if the event out needs to
be caught again. This behavior is probably less surprising and is arguably more consistent with the
atomicity of other interrupt management functions.


## System prerequisites

The uio_pruss kernel module must be loaded on your system. For the BeagleBone debian distribution,
it is available and loaded out-of-the-box with kernel 4.1.x and above on the **bone** kernel
images (in contrast to the **TI** images which include the remoteproc module instead).
You can install the bone image with the `--bone-kernel` or `--bone-rt-kernel` options, e.g.:

```text
$ cd /opt/scripts/tools/
$ git pull
$ sudo ./update_kernel.sh --bone-kernel --lts
```


## Installation

Just add the crate to your project's `Cargo.toml`:

```toml
[dependencies]
prusst = "0.1"
```

Note that Rust 1.9 or above is required.


## Cross-compilation

The native Rust toolchain runs flawlessly on the BeagleBone debian distribution, provided that
there is enough spare room (if HDMI is not used, the *console* debian image is a good option).
As can be expected though, compilation is a bit slowish.

Cross-compilation can solve this problem and is surprisingly simple, courtesy of
[rustup.rs](https://www.rustup.rs). Assuming a multiarch-enabled Ubuntu/Debian box, Rust can
be set up on the host machine with rustup.rs using this excellent
[cross-compilation guide](https://github.com/japaric/rust-cross).
If the host machine runs Debian Jessie, you may first need to add:

```text
deb http://emdebian.org/tools/debian/ jessie main
```

to your `/etc/apt/source.list` to be able to install the cross-toolchain.


## Hello world

```rust
extern crate prusst;

use prusst::{Pruss, IntcConfig, Sysevt, Evtout};
use std::fs::File;

fn main() {
    // Configure and get a view of the PRU subsystem.
    let mut pruss = Pruss::new(&IntcConfig::new_populated()).unwrap();
    
    // Get a handle to an event out before it is triggered.
    let irq = pruss.intc.register_irq(Evtout::E0);

    // Open, load and run a PRU binary.
    let mut file = File::open("hello.bin").unwrap();
    unsafe { pruss.pru0.load_code(&mut file).unwrap().run(); }
    
    // Wait for the PRU code from hello.bin to trigger an event out.
    irq.wait();
    
    // Clear the triggering interrupt.
    pruss.intc.clear_sysevt(Sysevt::S19);

    // Do nothing: the `pruss` destructor will stop any running code and release ressources.
    println!("We are done...");
}
```


## Examples

More advanced usage of the library (memory allocation, 2-ways communication with PRU, concurrent
management of IRQs, etc.) is demonstrated in the examples of the distribution tree.
If the library has been locally cloned, specific examples can be build as follows:

```text
$ cargo build --example blink
```

Before actually running the examples, however, it is first necessary to compile the associated
PRU programs. Assuming the PASM assembler is installed, all PRU programs can be compiled in one
go with:

```text
$ cd examples
$ sh make_pru_binaries
$ cd ..
```

It is also necessary to install the overlay ("cape") provided in the *examples* directory.
This overlay enables the PRU subsystem and two of the PRU-privileged GPIOs:
* `pr1_pru0_pru_r30_1`, a.k.a. BeagleBone pin P9_29
* `pr1_pru1_pru_r30_9`, a.k.a. BeagleBone pin P8_29

The overlay is compiled, installed and loaded as usual with:

```text
$ dtc -O dtb -o examples/prusst-examples-00A0.dtbo -b 0 -@ examples/prusst-examples.dts
$ sudo cp examples/prusst-examples-00A0.dtbo /lib/firmware
$ sudo sh -c "echo 'prusst-examples' > /sys/devices/platform/bone_capemgr/slots"
```

Finally, the examples must be run as root from a local directory containing a sub-directory
*examples* which itself contains the PRU executables:

```text
$ ls examples/*.bin
examples/blink_pru0.bin  examples/blink_pru1.bin  examples/pwm_generator.bin
$ sudo /path_to_executable/blink
```

**Important:** the *prusst-examples.dts* overlay is incompatible with HDMI on the BeagleBone Black.
To avoid any problem, no HDMI-enabling cape should be loaded at boot time (it is apparently not
enough to just unload them).


## License

This software is licensed under the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT), at your option.

Copyright (c) 2016 Serge Barral.

This library is named after French novelist Marcel Prusst (1871-1922). Well, almost.

