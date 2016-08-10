//! Flash LEDs on P9_29 and P8_29 5 times respectively at 2Hz and 3.33Hz using both PRUs.
//!
//! This example demonstrates:
//!
//! * concurrent code execution on two PRUs,
//! * concurrent monitoring within system threads on the host,
//! * discriminated, one-way event signaling from two PRUs to the host.
//!
//! Each PRU notifies the host processor every time its LED is flashed by triggering Evtout0 (PRU0)
//! and Evtout1 (PRU1). The host processor monitors both processes in dedicated threads.

extern crate prusst;
extern crate crossbeam;

use prusst::{Pruss, Intc, IntcConfig, Evtout, Sysevt, EvtoutIrq};
use std::fs::File;


fn blink_monitor(irq: EvtoutIrq, sysevt: Sysevt, my_name: &str, intc: &Intc) {
    // Let us know when the LED is turned on.
    for i in 1..6 {
        // Wait for the PRU to trigger an event out.
        irq.wait();
        println!("Blink {} from {}", i, my_name);

        // Clear the triggering interrupt and re-enable the host irq.
        intc.clear_sysevt(sysevt);
        intc.enable_host(irq.get_evtout());
    }
    
    // Wait for completion of the PRU code.
    irq.wait();
    intc.clear_sysevt(sysevt);
}


fn main() {
    // Get a view of the PRU subsystem.
    let mut pruss = match Pruss::new(&IntcConfig::new_populated()) {
        Ok(p) => p,
        Err(e) => match e {
            prusst::Error::AlreadyInstantiated
                => panic!("You can't instantiate more than one `Pruss` object at a time."),
            prusst::Error::PermissionDenied
                => panic!("You do not have permission to access the PRU subsystem: \
                           maybe you should log as root?"),
            prusst::Error::DeviceNotFound
                => panic!("The PRU subsystem could not be found: are you sure the `uio_pruss` \
                           module is loaded and supported by your kernel?"),
            prusst::Error::OtherDeviceError
                => panic!("An unidentified problem occured with the PRU subsystem: \
                           do you have a valid overlay loaded?")
        }
    };

    // Get handles to events out.
    let irq0 = pruss.intc.register_irq(Evtout::E0);
    let irq1 = pruss.intc.register_irq(Evtout::E1);
    
    // Open, load and run a PRU binary on each PRU core.
    let mut pru0_binary = File::open("examples/blink_pru0.bin").unwrap();
    unsafe { pruss.pru0.load_code(&mut pru0_binary).unwrap().run(); }
    let mut pru1_binary = File::open("examples/blink_pru1.bin").unwrap();
    unsafe { pruss.pru1.load_code(&mut pru1_binary).unwrap().run(); }
    
    // Launch a monitoring thread for each PRU core.
    crossbeam::scope(|scope| {
        scope.spawn(|| { blink_monitor(irq0, Sysevt::S19, "PRU0", &pruss.intc) } );
        scope.spawn(|| { blink_monitor(irq1, Sysevt::S20, "PRU1", &pruss.intc) } );
    });

    // Wait for completion on both PRUs.
    println!("Goodbye!");
}

