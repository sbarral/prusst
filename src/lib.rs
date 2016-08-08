//! A convenient Rust interface to the UIO kernel module for TI Programmable Real-time Unit
//! coprocessors, with roughly the same functionality as the
//! [C prussdrv library](https://github.com/beagleboard/am335x_pru_package)
//! but with a safer, rustic API that attempts to mitigate risks related to uninitialized or
//! invalid register states, use of freed memory, memory allocations conflicts etc.
//! 
//! 
//! # Design rationale
//! 
//! The design of the library exploits the Rust type system to reduce the risk of shooting onself
//! in the foot. Its architecture is meant to offer improved ergonomics compared to its C relative,
//! while operating at a similarly low level of abstraction and providing equivalent functionality.
//! 
//! Data-race safety is warranted by checking that only one `Pruss` instance (a view of the PRU
//! subsystem) is running at a time. The magic of the Rust borrowing rules will then _statically_
//! ensure, inter alia:
//! 
//! * the absence of memory aliasing for local and shared PRU RAM, meaning that a previously allocated
//! RAM segment may not be re-used before the data it contains is released,
//! 
//! * the impossibility to request code execution on a PRU core before the code has actually been
//! loaded,
//! 
//! * the impossibility to overwrite PRU code that is already loaded and still in use,
//! 
//! * the impossibility to concurrently modify the interrupt mapping.
//! 
//! Type safety also avoids many pitfalls associated with interrupt management. Unlike the C prussdrv
//! library, system events, host interrupt, events out and channels are all distinct types: they cannot
//! be misused or inadvertently switched in function calls. A related benefit is that the interrupt
//! management API is very self-explanatory.
//! 
//! Event handling is one of the few places where prusst requires the user to be more explicit
//! than the C prussdrv library. Indeed, the `prussdrv_pru_clear_event` function of the C driver
//! automatically re-enables an event out after clearing the triggering system event, which may wrongly
//! suggest that the combined clear-enable operation is thread-safe (it isn't). In contrast, prusst
//! mandates that both `Intc::clear_sysevt` and `Intc::enable_host` be called if the event out needs to
//! be caught again. This behavior is probably less surprising and is arguably more consistent with the
//! atomicity of other interrupt management functions.
//!
//!
//! # Hello world
//!
//! ```
//! extern crate prusst;
//! 
//! use prusst::{Pruss, IntcConfig, Sysevt, Evtout};
//! use std::fs::File;
//! 
//! fn main() {
//!     // Configure and get a view of the PRU subsystem.
//!     let mut pruss = Pruss::new(&IntcConfig::new_populated()).unwrap();
//!     
//!     // Get a handle to an event out before it is triggered.
//!     let irq = pruss.intc.register_irq(Evtout::E0);
//! 
//!     // Open, load and run a PRU binary.
//!     let mut file = File::open("hello.bin").unwrap();
//!     unsafe { pruss.pru0.load_code(&mut file).unwrap().run(); }
//!     
//!     // Wait for the PRU code from hello.bin to trigger an event out.
//!     irq.wait();
//!     
//!     // Clear the triggering interrupt.
//!     pruss.intc.clear_sysevt(Sysevt::S19);
//! 
//!     // Do nothing: the `pruss` destructor will stop any running code and release ressources.
//!     println!("We are done...");
//! }
//! ```

extern crate libc;

mod def;
mod error;
mod pubdef;
pub mod util;

use def::*;
pub use error::Error;
pub use pubdef::*;

use std::cmp::Eq;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read};
use std::marker::PhantomData;
use std::mem;
use std::ops::{BitOrAssign, Shl};
use std::ptr;
use std::result;
use std::sync::atomic::{AtomicBool, Ordering, ATOMIC_BOOL_INIT};



// A flag making sure that only one instance of the PRU subsystem is instantiated at a time.
static PRUSS_IS_INSTANTIATED: AtomicBool = ATOMIC_BOOL_INIT;



/// Result type for the PRU subsystem.
pub type Result<T> = result::Result<T, Error>;



/// Main interface to the PRU subsystem.
pub struct Pruss<'a> {
    _prumap: MemMap,
    _hostmap: MemMap,

    /// PRU interrupt controller
    pub intc: Intc,
    /// Program loader for PRU0
    pub pru0: PruLoader,
    /// Program loader for PRU1
    pub pru1: PruLoader,
    /// Data RAM for PRU0
    pub dram0: MemSegment<'a>,
    /// Data RAM for PRU1
    pub dram1: MemSegment<'a>,
    /// Shared data RAM
    pub dram2: MemSegment<'a>,
    /// Host memory
    pub hostram: MemSegment<'a>,
}

impl<'a> Pruss<'a> {
    /// Creates a PRU subsystem context, mapping all necessary PRU registers and memory.
    ///
    /// The interrupt controller is initialized with the provided mapping.
    pub fn new(intc_config: &IntcConfig) -> Result<Pruss<'a>> {
        // Enforce singleton instantiation.
        if PRUSS_IS_INSTANTIATED.swap(true, Ordering::Acquire) {
            return Err(Error::AlreadyInstantiated);
        }

        // Handy function to read the size of system devices.
        fn memsize(path: &str) -> io::Result<usize> {
            let mut f = try!(File::open(path));
            let mut buffer = String::new();
            try!(f.read_to_string(&mut buffer));
            Ok(usize::from_str_radix(&buffer[2..].trim(), 16).unwrap())
        };

        // Create memory mapped devices.
        let file = try!(SyncFile::new(PRUSS_DEVICE_PATH));
        let prumem_size = try!(memsize(UIO_PRUMEM_SIZE_PATH));
        let hostmem_size = try!(memsize(UIO_HOSTMEM_SIZE_PATH));
        let prumap = try!(MemMap::new(file.fd, prumem_size, 0));
        let hostmap = try!(MemMap::new(file.fd, hostmem_size, 1));

        // Create and initialize the interrupt controller.
        let mut intc = Intc::new(unsafe { prumap.base.offset(INTC_OFFSET as isize) as *mut u32 });
        intc.map_interrupts(intc_config);

        // Create the PRU code loaders.
        let pru0 =
            PruLoader::new(unsafe { prumap.base.offset(PRU0CTRL_OFFSET as isize) as *mut u32 },
                           unsafe { prumap.base.offset(IRAM0_OFFSET as isize) },
                           IRAM0_SIZE);
        let pru1 =
            PruLoader::new(unsafe { prumap.base.offset(PRU1CTRL_OFFSET as isize) as *mut u32 },
                           unsafe { prumap.base.offset(IRAM1_OFFSET as isize) },
                           IRAM1_SIZE);

        // Create memory views.
        let dram0 = MemSegment::new(prumap.base, DRAM0_OFFSET, DRAM0_OFFSET + DRAM0_SIZE);
        let dram1 = MemSegment::new(prumap.base, DRAM1_OFFSET, DRAM1_OFFSET + DRAM1_SIZE);
        let dram2 = MemSegment::new(prumap.base, DRAM2_OFFSET, DRAM2_OFFSET + DRAM2_SIZE);
        let hostram = MemSegment::new(hostmap.base, 0, hostmem_size);

        // Voila.
        Ok(Pruss {
            _prumap: prumap,
            _hostmap: hostmap,
            intc: intc,
            pru0: pru0,
            pru1: pru1,
            dram0: dram0,
            dram1: dram1,
            dram2: dram2,
            hostram: hostram,
        })
    }
}

impl<'a> Drop for Pruss<'a> {
    fn drop(&mut self) {
        // Stop instruction executions in both PRUs
        self.pru0.reset();
        self.pru1.reset();

        // Allow another PRU subsystem context to be instantiated.
        PRUSS_IS_INSTANTIATED.store(false, Ordering::Release);
    }
}

unsafe impl<'a> Send for Pruss<'a> {}

unsafe impl<'a> Sync for Pruss<'a> {}



/// The PRU interrupt controller.
pub struct Intc {
    intc_reg: *mut u32,
}

impl Intc {
    /// Creates a driver context with sane interrupt intc mapping defaults.
    fn new(intc_reg: *mut u32) -> Self {
        let intc = Intc { intc_reg: intc_reg };

        intc
    }

    /// Maps PRU interrupts according to the provided configuration.
    pub fn map_interrupts(&mut self, interrupts: &IntcConfig) {
        unsafe {
            // Set the polarity of system interrupts to high.
            ptr::write_volatile(self.intc_reg.offset(SIPR1_REG), 0xffffffff);
            ptr::write_volatile(self.intc_reg.offset(SIPR2_REG), 0xffffffff);

            // Clear all channel map registers and assign system events to channels.
            for cmrx in 0..NUM_CMRX {
                ptr::write_volatile(self.intc_reg.offset(CMR_REG + cmrx), 0);
            }
            for m in &interrupts.sysevt_to_channel_map {
                let cmrx = (m.sysevt >> 2) as isize;
                debug_assert!(cmrx < NUM_CMRX);
                let val = ptr::read_volatile(self.intc_reg.offset(CMR_REG + cmrx));
                ptr::write_volatile(self.intc_reg.offset(CMR_REG + cmrx),
                                    val | (m.channel as u32) << ((m.sysevt as u32 & 0b11) * 8));
            }

            // Clear all host map registers and assign channels to hosts.
            for hmrx in 0..NUM_HMRX {
                ptr::write_volatile(self.intc_reg.offset(HMR_REG + hmrx), 0);
            }
            for m in &interrupts.channel_to_host_map {
                let hmrx = (m.channel >> 2) as isize;
                debug_assert!(hmrx < NUM_HMRX);
                let val = ptr::read_volatile(self.intc_reg.offset(HMR_REG + hmrx));
                ptr::write_volatile(self.intc_reg.offset(HMR_REG + hmrx),
                                    val | (m.host as u32) << ((m.channel as u32 & 0b11) * 8));
            }

            // Set the type of system interrupts to pulse.
            ptr::write_volatile(self.intc_reg.offset(SITR1_REG), 0x0);
            ptr::write_volatile(self.intc_reg.offset(SITR2_REG), 0x0);

            // Enable and clear system events.
            let (mut mask1, mut mask2) = (0u32, 0u32);
            for se in &interrupts.sysevt_enable {
                match *se {
                    0...31 => mask1 |= 1u32 << se,
                    32...63 => mask2 |= 1u32 << (se - 32),
                    _ => unreachable!(),
                };
            }
            ptr::write_volatile(self.intc_reg.offset(ESR1_REG), mask1);
            ptr::write_volatile(self.intc_reg.offset(SECR1_REG), mask1);
            ptr::write_volatile(self.intc_reg.offset(ESR2_REG), mask2);
            ptr::write_volatile(self.intc_reg.offset(SECR2_REG), mask2);

            // Enable host interrupts.
            for h in &interrupts.host_enable {
                ptr::write_volatile(self.intc_reg.offset(HIEISR_REG), *h as u32);
            }
            ptr::write_volatile(self.intc_reg.offset(GER_REG), 0x1);
        }
    }
    
    /// Triggers a system event.
    pub fn send_sysevt(&self, sysevt: Sysevt) {
        unsafe {
            match sysevt as u8 {
                se @ 0...31 => ptr::write_volatile(self.intc_reg.offset(SRSR1_REG),
                                                   1u32 << se),
                se @ 32...63 => ptr::write_volatile(self.intc_reg.offset(SRSR2_REG),
                                                    1u32 << (se - 32)),
                _ => unreachable!(),
            };
        }
    }

    /// Clears a system event.
    pub fn clear_sysevt(&self, sysevt: Sysevt) {
        unsafe {
            ptr::write_volatile(self.intc_reg.offset(SICR_REG), sysevt as u32);
        }
    }

    /// Enables a system event.
    pub fn enable_sysevt(&self, sysevt: Sysevt) {
        unsafe {
            ptr::write_volatile(self.intc_reg.offset(EISR_REG), sysevt as u32 );
        }
    }

    /// Disables a system event.
    pub fn disable_sysevt(&self, sysevt: Sysevt) {
        unsafe {
            ptr::write_volatile(self.intc_reg.offset(EICR_REG), sysevt as u32 );
        }
    }

    /// Enables or re-enables a host interrupt.
    ///
    /// Beware: calling this function before the triggering system event was cleared will trigger
    /// the host interrupt again.
    pub fn enable_host<T: Into<Host>>(&self, host: T) {
        let host: Host = host.into();
        unsafe {
            ptr::write_volatile(self.intc_reg.offset(HIEISR_REG), host as u32 );
        }
    }

    /// Disables a host interrupt.
    pub fn disable_host<T: Into<Host>>(&self, host: T) {
        let host: Host = host.into();
        unsafe {
            ptr::write_volatile(self.intc_reg.offset(HIDISR_REG), host as u32 );
        }
    }

    /// Returns a synchronization primitive for event out host interrupts.
    ///
    /// Important: this function should be called before any corresponding event out is triggered.
    ///
    /// # Panics
    ///
    /// This function should not panic provided that the uio_pruss kernel module is loaded, which
    /// is theoretically guaranteed at this point since `Pruss` could not have been created
    /// otherwise.
    pub fn register_irq(&self, e: Evtout) -> EvtoutIrq {
        EvtoutIrq::new(e)
    }
}



/// PRU instruction code loader.
pub struct PruLoader {
    pructrl_reg: *mut u32,
    iram_base: *mut u8,
    iram_size: usize,
}

impl PruLoader {
    fn new(pructrl_reg: *mut u32, iram_base: *mut u8, iram_size: usize) -> PruLoader {

        PruLoader {
            pructrl_reg: pructrl_reg,
            iram_base: iram_base,
            iram_size: iram_size,
        }
    }

    /// Loads a binary of opcodes to the PRU without executing it.
    ///
    /// This function proceeds as follows:
    ///
    /// * a soft PRU reset is forced,
    /// * the code is written to the PRU instruction RAM.
    ///
    /// The code can be subsequently started and stopped using the returned `PruCode` handle.
    ///
    /// # Errors
    ///
    /// IO errors that may occur while reading the buffer are forwarded.
    /// If the buffer cannot be read entirely because the code does not fit into the instruction
    /// RAM, an error of the kind `ErrorKind::InvalidInput` is returned.
    // Disallow inlining: since `Read::read` does not use volatile stores, the compiler may
    // otherwise optimize away memory writes.
    #[inline(never)]
    pub fn load_code<R: Read>(&mut self, code: &mut R) -> io::Result<PruCode> {
        // Invoke a soft reset of the PRU to make sure no code is currently running.
        self.reset();
        // Write the code to the instruction RAM.
        let n: usize = try!(code.read( unsafe {
            std::slice::from_raw_parts_mut(self.iram_base, self.iram_size)
        }));
        // Make sure the whole buffer was read, otherwise return an InvalidInput error kind.
        match n {
            0 => {
                Err(io::Error::new(io::ErrorKind::InvalidInput,
                                   "size of PRU code exceeding instruction RAM capacity"))
            }
            _ => Ok(PruCode::new(self.pructrl_reg)),
        }
    }

    /// Resets the PRU.
    ///
    /// Invokes a soft reset by clearing the PRU control register.
    fn reset(&mut self) {
        unsafe {
            ptr::write_volatile(self.pructrl_reg, 0);
        }
    }
}



/// View of a contiguous memory segment.
///
/// The design of MemSegment is meant to allow allocation at arbitrary addresses while preventing
/// memory aliasing. This is achieved by allowing segments to be recursively split and by
/// borrowing segments upon object allocation, thus preventing further splitting and allocation
/// until the allocated object goes out of scope. For this reason, segments are neither copyable
/// nor clonable.
pub struct MemSegment<'a> {
    // It is necessary to keep the `from` index rather than offset the `base` pointer because
    // alignment must be checked when allocating memory for arbitrary types.
    base: *mut u8,
    from: usize,
    to: usize,
    _memory_marker: PhantomData<&'a [u8]>,
}

impl<'a> MemSegment<'a> {
    fn new<'b>(base: *mut u8, from: usize, to: usize) -> MemSegment<'b> {
        MemSegment {
            base: base,
            from: from,
            to: to,
            _memory_marker: PhantomData,
        }
    }
    
    /// Allocates an object at the beginning of the segment.
    ///
    /// # Panics
    ///
    /// This function will panic if the beginning of the segment is not properly aligned
    /// for type T or if the size of T exceeds its capacity.
    #[inline]
    pub fn alloc<T: Copy>(&mut self, source: T) -> &mut T {
        let target: &mut T = unsafe { self.alloc_uninitialized() };
        *target = source;

        target
    }

    /// Allocates an object at the begining of the segment without initializing it.
    ///
    /// This can save some unecessary initialization if the PRU is anyway going to initialize
    /// memory before it will be read by the host. In some cases, it can also be used to avoid
    /// trashing the stack with a large temporary initialization object if for some reason the
    /// compiler cannot inline the call to `alloc`.
    ///
    /// # Undefined Behavior
    ///
    /// Reading an uninitialized object is undefined behavior (even for Copy types).
    ///
    /// # Panics
    ///
    /// This function will panic if the beginning of the segment is not properly aligned
    /// for type T or if the size of T exceeds its capacity.
    pub unsafe fn alloc_uninitialized<T: Copy>(&mut self) -> &mut T {
        // Make sure the begining of the memory region is properly aligned for type T.
        assert!(self.from % mem::align_of::<T>() == 0);
        // Make sure the region is large enough to hold type T.
        assert!(self.to - self.from >= mem::size_of::<T>());

        &mut *(self.base.offset(self.from as isize) as *mut T)
    }

    /// Position at which the segment starts (in bytes).
    pub fn begin(&self) -> usize {
        self.from
    }

    /// Position at which the segment ends (in bytes).
    pub fn end(&self) -> usize {
        self.to
    }

    /// Splits the memory segment into two at the given byte position.
    ///
    /// Note that positions (addresses) are absolute and remain valid after the splitting
    /// operation. If for instance a segment is split at 0x00001000, the `begin` method of
    /// the second segment hence created will return 0x00001000 and not 0x00000000.
    pub fn split_at(&mut self, position: usize) -> (MemSegment, MemSegment) {
        assert!(position >= self.from && position <= self.to);
        (MemSegment {
            base: self.base,
            from: self.from,
            to: position,
            _memory_marker: PhantomData,
        },
         MemSegment {
            base: self.base,
            from: position,
            to: self.to,
            _memory_marker: PhantomData,
        })
    }
}

unsafe impl<'a> Send for MemSegment<'a> {}

unsafe impl<'a> Sync for MemSegment<'a> {}



/// PRU interrupt controller configuration.
///
/// A call to the `new_populated` method automatically initializes the data with the same defaults
/// as the PRUSS_INTC_INITDATA macro of the C prussdrv library. Alternatively, a blank-state
/// initialization data structure can be created with `new_empty` and then populated with the
/// dedicated methods.
#[derive(Clone)]
pub struct IntcConfig {
    sysevt_to_channel_map: Vec<SysevtToChannel>,
    channel_to_host_map: Vec<ChannelToHost>,
    sysevt_enable: Vec<u8>,
    host_enable: Vec<u8>,
}

impl IntcConfig {
    /// Constructs an empty PRU interrupt controller configuration.
    pub fn new_empty() -> IntcConfig {
        IntcConfig {
            sysevt_to_channel_map: Vec::new(),
            channel_to_host_map: Vec::new(),
            sysevt_enable: Vec::new(),
            host_enable: Vec::new(),
        }
    }

    /// Constructs a PRU interrupt controller configuration with a default mapping.
    ///
    /// The mapping reflects the one defined in the `PRUSS_INTC_INITDATA` C macro of the C
    /// prussdrv library, namely:
    ///
    /// * it maps:
    ///     - `Sysevt::S17` to `Channel::C1`,
    ///     - `Sysevt::S18` to `Channel::C0`,
    ///     - `Sysevt::S19` to `Channel::C2`,
    ///     - `Sysevt::S20` to `Channel::C3`,
    ///     - `Sysevt::S21` to `Channel::C0`,
    ///     - `Sysevt::S22` to `Channel::C1`,
    ///
    /// * it maps:
    ///     - `Channel::C0` to `Host::Pru0`,
    ///     - `Channel::C1` to `Host::Pru1`,
    ///     - `Channel::C2` to `Host::Evtout0`,
    ///     - `Channel::C3` to `Host::Evtout1`,
    ///
    /// * it enables:
    ///     - `Sysevt::S17`,
    ///     - `Sysevt::S18`,
    ///     - `Sysevt::S19`,
    ///     - `Sysevt::S20`,
    ///     - `Sysevt::S21`,
    ///     - `Sysevt::S22`,
    ///
    /// * it enables:
    ///     - `Host::Pru0`,
    ///     - `Host::Pru1`,
    ///     - `Host::Evtout0`,
    ///     - `Host::Evtout1`
    ///
    pub fn new_populated() -> IntcConfig {
        let mut config_data = Self::new_empty();
        config_data.map_sysevts_to_channels(&[(Sysevt::S17, Channel::C1),
                                            (Sysevt::S18, Channel::C0),
                                            (Sysevt::S19, Channel::C2),
                                            (Sysevt::S20, Channel::C3),
                                            (Sysevt::S21, Channel::C0),
                                            (Sysevt::S22, Channel::C1)]);
        config_data.map_channels_to_hosts(&[(Channel::C0, Host::Pru0),
                                          (Channel::C1, Host::Pru1),
                                          (Channel::C2, Host::Evtout0),
                                          (Channel::C3, Host::Evtout1)]);
        config_data.auto_enable_sysevts();
        config_data.auto_enable_hosts();

        config_data
    }

    /// Enables the specified system events.
    ///
    /// # Panics
    ///
    /// This will panic if a system event is enabled several times.
    pub fn enable_sysevts(&mut self, sysevts: &[Sysevt]) {
        let mut bitfield = BitField64::new(NUM_SYSEVTS);
        self.sysevt_enable = sysevts.iter()
            .map(|&sysevt| {
                assert!(bitfield.try_set(sysevt as u8));
                sysevt as u8
            })
            .collect();
    }

    /// Enables the specified host interrupts.
    ///
    /// # Panics
    ///
    /// This will panic if a host interrupt is enabled several times.
    pub fn enable_hosts(&mut self, hosts: &[Host]) {
        let mut bitfield = BitField32::new(NUM_HOSTS);
        self.host_enable = hosts.iter()
            .map(|&host| {
                assert!(bitfield.try_set(host as u8));
                host as u8
            })
            .collect()
    }

    /// Automatically enables system events that are already assigned to a channel.
    pub fn auto_enable_sysevts(&mut self) {
        self.sysevt_enable = self.sysevt_to_channel_map
            .iter()
            .map(|sysevt_to_channel| sysevt_to_channel.sysevt)
            .collect();
    }

    /// Automatically enables host interrupts that are already mapped to a channel.
    pub fn auto_enable_hosts(&mut self) {
        self.host_enable = self.channel_to_host_map
            .iter()
            .map(|channel_to_host| channel_to_host.host)
            .collect()
    }

    /// Assigns system events to channels.
    ///
    /// A channel can be targeted by several events but an event can be mapped to only one channel.
    ///
    /// # Panics
    ///
    /// This will panic if a system event is mapped to several channels simultaneously.
    pub fn map_sysevts_to_channels(&mut self, scmap: &[(Sysevt, Channel)]) {
        let mut bitfield = BitField64::new(NUM_SYSEVTS);
        self.sysevt_to_channel_map = scmap.iter()
            .map(|&(s, c)| {
                assert!(bitfield.try_set(s as u8));
                SysevtToChannel {
                    sysevt: s as u8,
                    channel: c as u8,
                }
            })
            .collect();
    }

    /// Assigns channel numbers to host interrupts.
    ///
    /// A host interrupt can be targeted by several channels but a channel can be mapped to only
    /// one host.
    ///
    /// # Panics
    ///
    /// This will panic if a channel is mapped to several hosts.
    pub fn map_channels_to_hosts(&mut self, chmap: &[(Channel, Host)]) {
        let mut bitfield = BitField32::new(NUM_CHANNELS);
        self.channel_to_host_map = chmap.iter()
            .map(|&(c, h)| {
                assert!(bitfield.try_set(c as u8));
                ChannelToHost {
                    channel: c as u8,
                    host: h as u8,
                }
            })
            .collect();
    }
}



/// Synchronization primitive that can be used to wait for an event out.
pub struct EvtoutIrq {
    file: File,
    event: Evtout,
}

impl EvtoutIrq {
    // This function should not panic as long as the UIO module is loaded.
    fn new(e: Evtout) -> EvtoutIrq {
        EvtoutIrq {
            file: File::open(format!("{}{}", EVTOUT_DEVICE_ROOT_PATH, e as usize)).unwrap(),
            event: e,
        }
    }

    /// Waits until the associated event out is triggered.
    ///
    /// # Panics
    ///
    /// This function should not panic as long as the UIO module is loaded, which is theoretically
    /// guaranteed at this point since `Pruss` could not have been created otherwise.
    pub fn wait(&self) -> u32 {
        let mut buffer = [0u8; 4];
        (&mut &(self.file)).read_exact(&mut buffer).unwrap();
        unsafe { mem::transmute::<[u8; 4], u32>(buffer) }
    }

    /// Returns the associated event out.
    pub fn get_evtout(&self) -> Evtout {
        self.event
    }
}



/// Handle to a binary code loaded in the PRU.
pub struct PruCode<'a> {
    pructrl_reg: *mut u32,
    _pructrl_marker: PhantomData<&'a mut u32>,
}

impl<'a> PruCode<'a> {
    fn new<'b>(pructrl_reg: *mut u32) -> PruCode<'b> {
        PruCode {
            pructrl_reg: pructrl_reg,
            _pructrl_marker: PhantomData,
        }
    }

    /// Executes the code loaded in the PRU.
    ///
    /// This function writes 1 to the enable bit of the PRU control register, which allows
    /// the loaded code to be started or, if it had been stopped, to resume its execution.
    ///
    /// # Safety
    ///
    /// This runs a binary code that has unrestricted access to pretty much all the processor memory
    /// and peripherals. What could possibly go wrong?
    pub unsafe fn run(&mut self) {
        // Set the enable bit of the PRU control register to start or resume code execution.
        ptr::write_volatile(self.pructrl_reg, 2);
    }

    /// Halts the execution of code running in the PRU.
    ///
    /// This function simply writes 0 to the enable bit of the PRU Control Register. If code was
    /// currently running, it will be stopped. Execution of the code can be resumed with a
    /// subsequent call to `run`.
    pub fn halt(&mut self) {
        // Clear the enable bit of the PRU control register to start or resume code execution
        // without resetting the PRU.
        unsafe {
            ptr::write_volatile(self.pructrl_reg, 1);
        }
    }

    /// Resets the PRU.
    ///
    /// Invokes a soft reset by clearing the PRU control register.
    pub fn reset(&mut self) {
        unsafe {
            ptr::write_volatile(self.pructrl_reg, 0);
        }
    }
}

unsafe impl<'a> Send for PruCode<'a> {}

unsafe impl<'a> Sync for PruCode<'a> {}



/// Connection from system event to channel
#[derive(Copy, Clone)]
struct SysevtToChannel {
    sysevt: u8,
    channel: u8,
}



/// Connection from channel to host
#[derive(Copy, Clone)]
struct ChannelToHost {
    channel: u8,
    host: u8,
}



/// A read-write file with synchronized I/O.
struct SyncFile {
    fd: libc::c_int,
}

impl SyncFile {
    fn new(path: &str) -> io::Result<SyncFile> {
        let fd = unsafe {
            libc::open(CString::new(path).unwrap().as_ptr(),
                       libc::O_RDWR | libc::O_SYNC)
        };
        match fd {
            err if err < 0 => Err(io::Error::from_raw_os_error(err as i32)),
            _ => Ok(SyncFile { fd: fd }),
        }
    }
}

impl Drop for SyncFile {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}



/// Memory-mapped file.
struct MemMap {
    base: *mut u8,
    size: usize,
}

impl MemMap {
    fn new(fd: libc::c_int, size: usize, page: isize) -> io::Result<MemMap> {
        unsafe {
            let base = libc::mmap(ptr::null_mut(),
                                  size as libc::size_t,
                                  libc::PROT_READ | libc::PROT_WRITE,
                                  libc::MAP_SHARED,
                                  fd,
                                  (PAGE_SIZE * page) as libc::off_t);
            if base == libc::MAP_FAILED {
                Err(io::Error::last_os_error())
            } else {
                Ok(MemMap {
                    base: base as *mut u8,
                    size: size,
                })
            }
        }
    }
}

impl Drop for MemMap {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.base as *mut libc::c_void, self.size as libc::size_t);
        }
    }
}



/// A bit field based on an unsigned type with a width of 256 at most.
#[derive(Copy, Clone)]
struct BitField<T> {
    bits: T,
    width: u8,
}

impl<T: Eq + BitOrAssign + From<u8> + Copy + Shl<u8, Output = T>> BitField<T> {
    /// Constructs a new bit field with the specified width.
    ///
    /// # Panics
    ///
    /// This will panic if the width does not fit within the underlying type.
    fn new(width: u8) -> Self {
        assert!((mem::size_of::<T>() * 8) >= width as usize);
        BitField {
            bits: 0u8.into(),
            width: width,
        }
    }

    /// Attempts to set the bit and returns true if succesful, i.e. if the bit was not already set.
    ///
    /// # Panics
    ///
    /// This will panic if the addressed bit is not witin the field width.
    fn try_set(&mut self, bit: u8) -> bool {
        assert!(bit < self.width);
        let mask: T = Into::<T>::into(1u8) << bit;
        let old = self.bits;
        self.bits |= mask;
        old != self.bits
    }
}

type BitField32 = BitField<u32>;

type BitField64 = BitField<u64>;
