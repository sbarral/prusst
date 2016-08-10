// Flash a LED on P8_29 (pru1_pru_r30_9) 5 times at 3.33Hz.

.origin 0
.entrypoint start

// Flash led 5 times every 300ms with a 50% duty cycle.
#define INS_PER_MS    200 * 1000
#define ON_DURATION   150 * INS_PER_MS
#define OFF_DURATION  150 * INS_PER_MS
#define NB_BLINKS     5

// Assume that SYSEVT19 is mapped to EVTOUT0.
#define PRU1_ARM_SYSEVT 20

start:
    mov  r1, NB_BLINKS

start_loop:
    set  r30.t9              // turn LED on
    mov  r31.b0, 32 | (PRU1_ARM_SYSEVT - 16) // notify LED blink to host
    mov  r0, ON_DURATION

delay_on:
    sub  r0, r0, 1
    qbne delay_on, r0, 0

led_off:
    clr  r30.t9              // turn LED off
    mov  r0, OFF_DURATION

delay_off:
    sub  r0, r0, 1
    qbne delay_off, r0, 0

    sub  r1, r1, 1
    qbne start_loop, r1, 0

    mov  r31.b0, 32 | (PRU1_ARM_SYSEVT - 16) // notify completion to host
    halt

