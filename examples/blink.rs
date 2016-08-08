//! Flash a LED on P9_29 (pru0_pru_r30_1) 5 times at 2Hz.
//!
//! The PRU code notifies the host processor every time the LED is flashed by triggering Evtout0.
//! The 6-th event out is interpreted as a completion notification. 

extern crate prusst;

use prusst::{Pruss, IntcConfig, Evtout, Sysevt};
use std::fs::File;

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
    
    // Get a handle to an event out.
    let irq = pruss.intc.register_irq(Evtout::E0);

    // Open and load a PRU binary.
    let mut pru_binary = File::open("examples/blink_pru0.bin").unwrap();
    unsafe { pruss.pru0.load_code(&mut pru_binary).unwrap().run(); }
    
    // Let us know when the LED is turned on.
    for i in 1..6 {
        // Wait for the PRU to trigger the event out.
        irq.wait();
        println!("Blink {}", i);

        // Clear the triggering interrupt and re-enable the host irq.
        pruss.intc.clear_sysevt(Sysevt::S19);
        pruss.intc.enable_host(Evtout::E0);
    }

    // Wait for completion of the PRU code.
    irq.wait();
    println!("Goodbye!");
}
