//! 8-bit PWM sine wave generation on P9_29 (pru0_pru_r30_1).
//!
//! This example demonstrates:
//!
//! * PRU memory allocation,
//! * communication of data from host to PRU
//! * communication of data from PRU to host (and use of VolatileCell),
//! * two-way event signaling between host and PRU.
//!
//! The PRU code is designed to take exactly 10 PRU cycles per PWM subsamples independently of the
//! program flow. Since each PWM cycle is divided into 255 subsamples and the PRU is clocked at
//! 200MHz, the PWM frequency is approximately 78431Hz.
//!
//! The duty cycle of each subsample of the sine wave is specified as a fraction N/255 with
//! 0<=N<=255. The values N are stored in a 256-elements array allocated at 0x00000100 of the
//! local RAM of PRU0.
//! Other data exchanged between the host and the PRU is stored in a control struct allocated
//! at 0x00000000 of the local RAM of PRU0. The wave frequency is controlled by variable
//! `ctrl.sample_length` which sets the number of PWM sub-samples to be generated for any of the
//! 256 samples of the sine wave. Upon completion, the PRU writes the total number of subsamples
//! actually generated in variable `ctrl.sample_count`.

extern crate prusst;

use prusst::{Pruss, IntcConfig, Evtout, Sysevt};
use prusst::util::VolatileCell;

use std::fs::File;
use std::f32::consts;
use std::io::{self, Write};
use std::time::Duration;
use std::thread;

const ARRAY_BASE: usize = 0x100;
const NB_SAMPLES: usize = 256;
const PRU_FREQUENCY: f32 = 200e6;
const TICKS_PER_SUBSAMPLE: u32 = 10; // nb of PRU clock ticks per sub-sample
const MIN_SAMPLE_LENGTH: u32 = 255; // min nb of sub-samples per wave sample


#[repr(C)]
#[derive(Copy, Clone)]
struct Ctrl {
    sample_count: VolatileCell<u32>, // number of full wavelengths already generated
    sample_length: u32, // number of sub-samples per wave sample
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
    
    // Split the PRU data RAM into two segments, then allocate the control struct and allocate the
    // waveform array; note that the array could have been a field of the Ctrl struct to avoid
    // separate allocation, but this way we have learned how to that and as a bonus we get a
    // nice round address for the array too...
    let (mut bank1, mut bank2) = pruss.dram0.split_at(ARRAY_BASE);
    let ctrl = bank1.alloc(Ctrl { sample_count: VolatileCell::new(0), sample_length: 0 });
    let wave = unsafe { bank2.alloc_uninitialized::<[u8; NB_SAMPLES]>() };

    // Ask for the amplitude and frequency of the wave.
    let max_frequency: f32 = PRU_FREQUENCY /
        (TICKS_PER_SUBSAMPLE as f32 * MIN_SAMPLE_LENGTH as f32 * NB_SAMPLES as f32);
    let amplitude:  f32 = get_input("Amplitude [%]",  0.0, 100.0)/100.0;
    let frequency:  f32 = get_input("Frequency [Hz]", 0.0, max_frequency);
    let duration:   f32 = get_input("Duration [s]",   0.0, 60.0);
    
    // Compute the number of sub-sampling cycles per sample required for the requested frequency.
    let sampling_frequency: f32 = frequency * (NB_SAMPLES as f32);
    let sampling_period:    f32 = 1.0/sampling_frequency;
    let ticks_per_sample:   f32 = sampling_period * PRU_FREQUENCY; // PRU clock ticks per sample
    let sample_length:      u32 = (ticks_per_sample / (TICKS_PER_SUBSAMPLE as f32)).round() as u32;

    // Write the cycle length and generate the sine wave data.
    println!("\nGenerating wave with frequency {} Hz", PRU_FREQUENCY /
        (TICKS_PER_SUBSAMPLE as f32 * sample_length as f32 * NB_SAMPLES as f32) );
    ctrl.sample_length = sample_length;
    for (i, val) in wave.iter_mut().enumerate() {
        let phi = (i as f32)/(NB_SAMPLES as f32)*(2.0 * consts::PI);
        *val = (0.5*(1.0 - phi.cos()) * amplitude * 255.0).round() as u8;
    }

    // Get a handle to an event out.
    let irq = pruss.intc.register_irq(Evtout::E0);

    // Open and load a PRU binary.
    let mut pru_binary = File::open("examples/pwm_generator.bin").unwrap();
    unsafe { pruss.pru0.load_code(&mut pru_binary).unwrap().run(); }
    
    // Request a PRU halt when the duration has elapsed.
    thread::sleep(Duration::new(duration.floor() as u64,
                                (duration.fract()*1e9).floor() as u32));
    pruss.intc.send_sysevt(Sysevt::S21);
    
    // Await for acknowledgement from PRU.
    irq.wait();
    pruss.intc.clear_sysevt(Sysevt::S19);

    println!("{} PWM samples have been generated", ctrl.sample_count.get());
}


fn get_input(prompt: &str, min: f32, max: f32) -> f32 {
    loop {
        print!("{} ({}-{}): ", prompt, min, max);
        io::stdout().flush().unwrap();

        let mut val = String::new();
        io::stdin().read_line(&mut val).expect("failed to read input");

        if let Ok(val) = val.trim().parse::<f32>() {
            if val>=min && val<=max {
                return val;
            } else {
                println!("the input should be between {} and {}", min, max);
            }
        } else {
            println!("not a number");
        }
    }
}

