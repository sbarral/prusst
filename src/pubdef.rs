
use std::mem;

/// A PRU-generated system event.
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Sysevt {
    S0,
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    S12,
    S13,
    S14,
    S15,
    S16,
    S17,
    S18,
    S19,
    S20,
    S21,
    S22,
    S23,
    S24,
    S25,
    S26,
    S27,
    S28,
    S29,
    S30,
    S31,
    S32,
    S33,
    S34,
    S35,
    S36,
    S37,
    S38,
    S39,
    S40,
    S41,
    S42,
    S43,
    S44,
    S45,
    S46,
    S47,
    S48,
    S49,
    S50,
    S51,
    S52,
    S53,
    S54,
    S55,
    S56,
    S57,
    S58,
    S59,
    S60,
    S61,
    S62,
    S63,
}


/// A channel to which system interrupts can be mapped.
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Channel {
    C0,
    C1,
    C2,
    C3,
    C4,
    C5,
    C6,
    C7,
    C8,
    C9,
}



/// A host to which channels can be mapped.
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Host {
    Pru0,
    Pru1,
    Evtout0,
    Evtout1,
    Evtout2,
    Evtout3,
    Evtout4,
    Evtout5,
    Evtout6,
    Evtout7,
}



/// An event out.
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Evtout {
    E0,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
}

impl Into<Host> for Evtout
{
    fn into(self) -> Host {
        unsafe { mem::transmute(self as u8 + 2) }
    }
}

