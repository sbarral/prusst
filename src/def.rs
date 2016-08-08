
// System paths
pub const PRUSS_DEVICE_PATH: &'static str = "/dev/uio0";
pub const EVTOUT_DEVICE_ROOT_PATH: &'static str = "/dev/uio";
pub const UIO_PRUMEM_SIZE_PATH: &'static str = "/sys/class/uio/uio0/maps/map0/size";
pub const UIO_HOSTMEM_SIZE_PATH: &'static str = "/sys/class/uio/uio0/maps/map1/size";


// Number of hosts, channels and events
pub const NUM_SYSEVTS: u8 = 64;
pub const NUM_CHANNELS: u8 = 10;
pub const NUM_HOSTS: u8 = 10;


// Memory offsets relative to PRU memory base and sizes of PRU memory regions
pub const DRAM0_OFFSET: usize = 0x00000;
pub const DRAM1_OFFSET: usize = 0x02000;
pub const DRAM2_OFFSET: usize = 0x10000;
pub const INTC_OFFSET: usize = 0x20000;
pub const PRU0CTRL_OFFSET: usize = 0x22000;
pub const PRU1CTRL_OFFSET: usize = 0x24000;
pub const IRAM0_OFFSET: usize = 0x34000;
pub const IRAM1_OFFSET: usize = 0x38000;


// Size of PRU memory regions
pub const DRAM0_SIZE: usize = 0x02000; // 8kB
pub const DRAM1_SIZE: usize = 0x02000; // 8kB
pub const DRAM2_SIZE: usize = 0x03000; // 12kB
pub const IRAM0_SIZE: usize = 0x02000; // 8kB
pub const IRAM1_SIZE: usize = 0x02000; // 8kB


// Memory offsets expressed as 32-bit words relative to the interrupt controller memory base
pub const GER_REG: isize = 0x004;

pub const SICR_REG: isize = 0x009;

pub const EISR_REG: isize = 0x00a;

pub const EICR_REG: isize = 0x00b;

pub const HIEISR_REG: isize = 0x00d;

pub const HIDISR_REG: isize = 0x00e;

pub const SRSR1_REG: isize = 0x080;
pub const SRSR2_REG: isize = 0x081;

pub const SECR1_REG: isize = 0x0a0;
pub const SECR2_REG: isize = 0x0a1;

pub const ESR1_REG: isize = 0x0c0;
pub const ESR2_REG: isize = 0x0c1;

pub const CMR_REG: isize = 0x100;

pub const HMR_REG: isize = 0x200;

pub const SIPR1_REG: isize = 0x340;
pub const SIPR2_REG: isize = 0x341;

pub const SITR1_REG: isize = 0x360;
pub const SITR2_REG: isize = 0x361;


// Number of sub-registers
pub const NUM_CMRX: isize = 16;
pub const NUM_HMRX: isize = 3;


// Misc
pub const PAGE_SIZE: isize = 4096;
