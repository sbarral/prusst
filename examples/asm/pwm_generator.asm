; 8-bit PWM periodic signal generation on P9_29 (pru0_pru_r30_1).
;
; The code is designed to take exactly 10 PRU cycles per PWM subsample
; independently of the program flow. Since each PWM cycle is divided into 255
; subsamples and the PRU is clocked at 200MHz, the PWM frequency is
; approximately 78431Hz.
; The duty cycle is specified as a fraction N/255 with 0<=N<=255.
; Since the sub-sampling frequency is fixed, the signal frequency is
; controlled by updating the sample's duty cycle N value based on a sample
; length counter, which at every sub-sampling cycle is incremented and
; compared to the user-specified sample length.

WAIT_1 .macro
       lsl r0, r0, 0
       .endm

WAIT_3 .macro
       WAIT_1
       WAIT_1
       WAIT_1
       .endm


PRU0_ARM_SYSEVT .set 19
ARM_PRU0_SYSEVT .set 21
PRU0_IRQ_BIT .set 30
SICR_OFFSET .set 0x24
CONST_INTC .set c0


    .global _c_int00

_c_int00:
    ldi   r10, 0x000            ; r10: RAM base address
    ldi   r11, 0x100            ; r11: base address of the samples array
    lbbo  &r1, r10, 4, 4        ; r1: sample length (nb of sub-samples)
    ldi   r2, 0                 ; r2: total sample count (r2.b0 is current sample)
    ldi   r3.b0, 1              ; r3: pulse count (sub-sample, 1-255)
    qba   sampling_loop_init

sampling_loop_start:            ; [1 cycle]
    add   r2, r2, 1             ; increment total sample count

sampling_loop_init:             ; [4 cycles]
	lbbo  &r4.b0, r11, r2.b0, 1 ; r4: pulse length (nb of sub-samples, 0-255)
    ldi   r5, 1                 ; r5: sample length counter (nb of sub-samples)
    
subsampling_loop_start:         ; [5 cycles]
    add   r6, r3.b0, r4.b0      ; r6.t8 (carry bit) = (r3.b0 + r4.b0) > 255
    lsl   r30.b0, r6.b1, 1      ; r30.t1 = (r3.b0 + r4.b0) > 255
    add   r3.b0, r3.b0, 1       ; increment pulse count modulo 256
    max   r3.b0, r3.b0, 1       ; rewind at 1 rather than 0
    qbge  sampling_loop_start, r1, r5 ; update sample if r5 >= r1

increment:                      ; [5 cycles]
    add   r5, r5, 1             ; increment sample length counter
    WAIT_3                      ; match outer loop overhead
    qbbc  subsampling_loop_start, r31, PRU0_IRQ_BIT ; irq received from host?

end:
    ldi   r7, ARM_PRU0_SYSEVT
    sbco  &r7, CONST_INTC, SICR_OFFSET, 2 ; clear system event triggered by host
    sbbo  &r2, r10, 0, 4        ; store total sample count to data RAM
    ldi   r31.b0, 32 | (PRU0_ARM_SYSEVT - 16) ; notify completion to host
    halt

