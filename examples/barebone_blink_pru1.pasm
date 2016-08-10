// Flash the BeagleBone USR2 LED 5 times at 3.33Hz.

.origin 0
.entrypoint start

// On-board LEDs
#define GPIO1 0x4804C000
#define CLEARDATAOUT_OFFSET 0x190
#define SETDATAOUT_OFFSET   0x194
#define USR0_GPIO1_21     1 << 21
#define USR1_GPIO1_22     1 << 22
#define USR2_GPIO1_23     1 << 23
#define USR3_GPIO1_24     1 << 24

// Settings for a 300ms period and 50% duty cycle.
#define INS_PER_MS    200 * 1000
#define ON_DURATION   150 * INS_PER_MS
#define OFF_DURATION  150 * INS_PER_MS
#define NB_BLINKS     5

// Assume that SYSEVT19 is mapped to EVTOUT0.
#define PRU0_ARM_SYSEVT 20

// Constant registers
#define CPRUCFG  c4
#define CPRUDRAM c24


start:
    lbco r0, CPRUCFG, 4, 4   // read SYSCFG
    clr  r0.t4               // clear SYSCFG[STANDBY_INIT]
    sbco r0, CPRUCFG, 4, 4   // enable OCP master port
    
    mov  r1, NB_BLINKS
    mov  r2, GPIO1 + SETDATAOUT_OFFSET
    mov  r3, GPIO1 + CLEARDATAOUT_OFFSET
    mov  r4, USR2_GPIO1_23

start_loop:
    sbbo r4, r2, 0, 4        // turn LED on
    mov  r31.b0, 32 | (PRU0_ARM_SYSEVT - 16) // notify LED blink to host
    mov  r0, ON_DURATION

delay_on:
    sub  r0, r0, 1
    qbne delay_on, r0, 0

led_off:
    sbbo r4, r3, 0, 4        // turn LED off
    mov  r0, OFF_DURATION

delay_off:
    sub  r0, r0, 1
    qbne delay_off, r0, 0

    sub  r1, r1, 1
    qbne start_loop, r1, 0

    mov  r31.b0, 32 | (PRU0_ARM_SYSEVT - 16) // notify completion to host
    halt

