# prusst

[![Build Status](https://travis-ci.org/sbarral/prusst.svg?branch=master)](https://travis-ci.org/sbarral/prusst)
[![Version](http://meritbadge.herokuapp.com/prusst)](https://crates.io/crates/prusst)

A convenient Rust interface to the UIO kernel module for TI Programmable
Real-time Unit coprocessors found among others on the BeagleBone development
boards. It provides roughly the same functionality as the [C prussdrv
library](https://github.com/beagleboard/am335x_pru_package) but with a safer,
rustic API that attempts to mitigate risks related to uninitialized or invalid
register states, use of freed memory, memory allocations conflicts etc.


## Documentation

The API documentation lives [here](https://sbarral.github.io/prusst-doc/prusst/).


## Background

PRUs (Programmable Real-time Units) are RISC cores integrated into some TI
processors such as the AM335x that powers the BeagleBone::{White, Black, Green}
development boards. They are what sets the BeagleBone apart from other popular
single-board computers, allowing real-time process control without the
complexity associated with an external co-processor.

PRUs have direct access to some general purpose I/O pins as well as indirect
access to memory and peripherals via an interconnect bus. Their predictable
single-cycle instruction execution and absence of pipe-lining or caching makes
them especially suitable for real-time processing.

Since the PRU assembly language is simple and enables total control of the
execution timing, critical real-time tasks can be programmed directly in
assembly for the PRU, which cooperates with the host processor for the
heavy-lifting (pre and post-processing, communication etc.).

There currently exist two options to communicate with the PRU from the host
processor, namely the UIO and the remoteproc kernel modules. The UIO kernel
module offers a low-level access to the PRU subsystem and is generally better
suited for pure assembler PRU code with accurate execution timing based on
instruction cycle count. The remoteproc kernel module is in turn better suited
for higher-level and somewhat more portable PRU programming in C but less
suitable for deterministic, tight real-time control due to the overhead of the
message-passing mechanism.

This library provides a relatively simple abstraction over the UIO
kernel module which makes it easy to perform common operations such as
executing code on the PRU, transferring data between the PRU and the host
processor or triggering/waiting for system events.


## Design rationale

The design of the library exploits the Rust type system to reduce the risk of
shooting onself in the foot. Its architecture is meant to offer improved
ergonomics compared to its C relative, while operating at a similarly low level
of abstraction and providing equivalent functionality.

Data-race safety is warranted by checking that only one `Pruss` instance (a
view of the PRU subsystem) is running at a time. The magic of the Rust
borrowing rules will then _statically_ ensure, inter alia:

* the absence of memory aliasing for local and shared PRU RAM, meaning that a
  previously allocated RAM segment may not be re-used before the data it
  contains is released,

* the impossibility to request code execution on a PRU core before the code has
  actually been loaded,

* the impossibility to overwrite PRU code that is already loaded and still in
  use,

* the impossibility to concurrently modify the interrupt mapping.

Type safety also avoids many pitfalls associated with interrupt management.
Unlike the C prussdrv library, system events, host interrupt, events out and
channels are all distinct types: they cannot be misused or inadvertently
switched in function calls. A related benefit is that the interrupt management
API is very self-explanatory.

Event handling is one of the few places where prusst requires the user to be
more explicit than the C prussdrv library. Indeed, the
`prussdrv_pru_clear_event` function of the C driver automatically re-enables an
event out after clearing the triggering system event, which may wrongly suggest
that the combined clear-enable operation is thread-safe (it isn't). In
contrast, prusst mandates that both `Intc::clear_sysevt` and
`Intc::enable_host` be called if the event out needs to be caught again. This
behavior is probably less surprising and is arguably more consistent with the
atomicity of the other interrupt management functions.


## System prerequisites

The UIO kernel module must be loaded on your system.  Mainline debian "Stretch"
distributions from May 2017 onward can be easily configured to access the PRU
via either remoteproc or UIO.

The UIO overlay must first be selected by editing */boot/uEnv.txt*, e.g.:

```text
...
# uboot_overlay_pru=/lib/firmware/AM335X-PRU-RPROC-4-4-TI-00A0.dtbo
...
uboot_overlay_pru=/lib/firmware/AM335X-PRU-UIO-00A0.dtbo
...
```

Then, the remoteproc modules must be blacklisted by editing or creating
*/etc/modprobe.d/pruss-blacklist.conf* with the following content:

```text
blacklist pruss
blacklist pruss_intc
blacklist pru_rproc
```

If all goes well, the UIO kernel modules should show up after reboot:

```text
$ lsmod | grep uio
uio_pruss               4629  0
uio_pdrv_genirq         4243  0
uio                    11100  2 uio_pruss,uio_pdrv_genirq
```

## Installation

Just add the crate to your project's *Cargo.toml*:

```toml
[dependencies]
prusst = "1.0"
```

> **prusst 1.0 requires rust 1.21 or above.**
>
> If you cannot use rust 1.21, no worry: just use prusst 0.1! I am sure you
> will be fine.
>
> The API hasn't changed: the version bump is mostly to indicate that the
> original design has withstood the test of time.
> Although 1.0 does contain a fix that was waiting for rust 1.21 (support
> for compiler barriers), the issue is quite hypothetical and unlikely to
> ever affect you.



## Cross-compilation

The native Rust toolchain runs flawlessly on the BeagleBone debian
distribution, provided that there is enough spare room. The *IoT* debian image
is a good option.

As can be expected though, compilation is a bit slowish.

Cross-compilation can save you a lot of pain and is surprisingly simple,
courtesy of [rustup.rs](https://www.rustup.rs).
For a step-by-step procedure to install an ARM v7 target with rustup.rs, see the
[rust cross-compilation bible](https://github.com/japaric/rust-cross).


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

More advanced usage of the library is demonstrated in the [examples](examples),
such as PRU RAM allocation, 2-ways communication with PRU, concurrent
management of IRQs, etc.

Assuming the prusst crate has been locally cloned, the first step before
running the examples is to compile the PRU assembler code. This may be done
with either the `pasm` assembler or the `clpru` compiler.  The former is no
longer maintained but can be retrieved as part of the
[am335x-pru-package](https://github.com/beagleboard/am335x_pru_package).  
The best option today, however, is probably to use the `clpru` compiler which
is now bundled in the mainline BeagleBone distribution and actively
maintained by TI. Note that the assembler syntax differs slightly so two
versions of each PRU example code are provided, one for `pasm`
(*.pasm* files in the *pasm* directory) and one for `clpru` (*.asm* files in
the *asm* directory).

To build the example PRU binaries with `clpru` do:

```text
$ cd examples
$ make asm
```

Likewise, to build the binaries with `pasm` do:

```text
$ cd examples
$ make pasm
```

If you use a BeagleBone with *cape-universal* enabled (which should be the case
for relatively new distributions), the PRU is already configured by default.
You may then right away blink the USR BeagleBone LEDs with the *barebone_blink*
and *barebone_parallel_blink* examples (to be run from within the crate root
directory, otherwise the PRU binaries will not be found):

```text
$ cargo run --example barebone_blink
```

The *pwm_generator* example needs a bit more setup as it uses a PRU-privileged
GPIO (pr1_pru0_pru_r30_1, a.k.a.  pin P9_29 on the BeagleBone).

On the BeagleBone Black and other HDMI-equipped boards, all PRU-privileged pins
are by default reserved for HDMI so it is necessary to first disable HDMI in
*/boot/uEnv.txt* by uncommenting the following line (note that this actually
disables both HDMI video *and* audio):

```text
disable_uboot_overlay_video=1
```

After re-boot, pin P9_29 can be configured as a PRU out pin with:

```text
$ config-pin P9.29 pruout
``` 

and the PWM example is ready to be fired:

```text
$ cargo run --example pwm_generator
```

This will generate an 8-bit PWM sine wave with a configurable frequency and
amplitude, using a constant 78431Hz PWM switching frequency.

If you do not have a BeagleBone and/or a PRU-enabling overlay, you may install
the *prusst-examples* overlay provided in the *examples* directory. This
overlay enables the PRU subsystem and BeagleBone pin P9_29 as a PRU out pin.

The overlay is compiled and installed as follows:

```text
$ dtc -O dtb -o examples/prusst-examples-00A0.dtbo -b 0 -@ examples/prusst-examples.dts
$ sudo cp examples/prusst-examples-00A0.dtbo /lib/firmware
```

To activate it, edit */boot/uEnv.txt*.


> IMPORTANT: if you use the the *prusst-examples* overlay, note that it is incompatible with HDMI
> on the BeagleBone Black and other HDMI-equipped boards.
> To avoid any problem, no HDMI-enabling cape should be loaded at boot time.


## License

This software is licensed under the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT), at your option.

Copyright (c) 2017 Serge Barral.

This library is named after French novelist Marcel Prusst (1871-1922). Well, almost.

