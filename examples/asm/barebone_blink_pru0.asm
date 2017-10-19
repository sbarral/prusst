; Flash the BeagleBone USR1 LED 5 times at 2Hz.

; On-board LEDs
GPIO1 .set 0x4804C000
CLEARDATAOUT_OFFSET .set 0x190
SETDATAOUT_OFFSET .set 0x194
USR0_GPIO1_21 .set 1 << 21
USR1_GPIO1_22 .set 1 << 22
USR2_GPIO1_23 .set 1 << 23
USR3_GPIO1_24 .set 1 << 24

; Settings for a 500ms period and 50% duty cycle.
INS_PER_MS .set 200 * 1000
ON_DURATION .set 250 * INS_PER_MS
OFF_DURATION .set 250 * INS_PER_MS
NB_BLINKS .set 5

; Assume that SYSEVT19 is mapped to EVTOUT0.
PRU0_ARM_SYSEVT .set 19

; Constant registers
CPRUCFG .set c4
CPRUDRAM .set c24


    .global _c_int00

_c_int00:
    lbco  &r0, CPRUCFG, 4, 4   ; read SYSCFG
    clr   r0, r0.t4            ; clear SYSCFG[STANDBY_INIT]
    sbco  &r0, CPRUCFG, 4, 4   ; enable OCP master port
    
    ldi   r1, NB_BLINKS
    ldi32 r2, GPIO1 + SETDATAOUT_OFFSET
    ldi32 r3, GPIO1 + CLEARDATAOUT_OFFSET
    ldi32 r4, USR1_GPIO1_22

start_loop:
    sbbo  &r4, r2, 0, 4        ; turn LED on
    ldi   r31.b0, 32 | (PRU0_ARM_SYSEVT - 16) ; notify LED blink to host
    ldi32 r0, ON_DURATION

delay_on:
    sub   r0, r0, 1
    qbne  delay_on, r0, 0

led_off:
    sbbo  &r4, r3, 0, 4        ; turn LED off
    ldi32 r0, OFF_DURATION

delay_off:
    sub   r0, r0, 1
    qbne  delay_off, r0, 0

    sub   r1, r1, 1
    qbne  start_loop, r1, 0

    ldi   r31.b0, 32 | (PRU0_ARM_SYSEVT - 16) ; notify completion to host
    halt

