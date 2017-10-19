-cr

MEMORY
{
    PAGE 0:
        PRU_IMEM: o = 0x00000000 l = 0x00002000 /* 8kB PRU0 Instruction RAM */
}

SECTIONS
{
    .text:_c_int00 > 0x0, PAGE 0
    .text > PRU_IMEM, PAGE 0
}
