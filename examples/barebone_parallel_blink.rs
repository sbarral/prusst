//! Flash the BeagleBone USR1 and USR2 LEDs 5 times at 2Hz and 3.33Hz using both PRUs.
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
use std::io::Write;


static LED1_TRIGGER_PATH: &'static str = "/sys/class/leds/beaglebone:green:usr1/trigger";
static LED1_DEFAULT_TRIGGER: &'static str = "mmc0";
static LED2_TRIGGER_PATH: &'static str = "/sys/class/leds/beaglebone:green:usr2/trigger";
static LED2_DEFAULT_TRIGGER: &'static str = "cpu0";


fn echo(value: &str, path: &str) {
    let mut file = File::create(path).unwrap();
    file.write_all(value.as_bytes()).unwrap();
}


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
    
    // Open and load the PRU binaries on each PRU core.
    let mut pru0_binary = File::open("examples/barebone_blink_pru0.bin").unwrap();
    let mut pru1_binary = File::open("examples/barebone_blink_pru1.bin").unwrap();
    
    // Temporarily take control of the LEDs.
    echo("none", LED1_TRIGGER_PATH);
    echo("none", LED2_TRIGGER_PATH);

    // Run the PRU binaries.
    unsafe { pruss.pru0.load_code(&mut pru0_binary).unwrap().run(); }
    unsafe { pruss.pru1.load_code(&mut pru1_binary).unwrap().run(); }

    // Launch a monitoring thread for each PRU core.
    crossbeam::scope(|scope| {
        scope.spawn(|| { blink_monitor(irq0, Sysevt::S19, "PRU0", &pruss.intc) } );
        scope.spawn(|| { blink_monitor(irq1, Sysevt::S20, "PRU1", &pruss.intc) } );
    });

    // Wait for completion on both PRUs.
    println!("Goodbye!");
    
    // Restore default LEDs statuses.
    echo(LED1_DEFAULT_TRIGGER, LED1_TRIGGER_PATH);
    echo(LED2_DEFAULT_TRIGGER, LED2_TRIGGER_PATH);
}

