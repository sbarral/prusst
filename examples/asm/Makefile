
PRU_ASM = clpru
PRU_HEX = hexpru
ASM_FLAGS = --silicon_version=3

LINKER_CMD = am335x_pru_imem.cmd

INSTALL_DIR = ..


.PHONY: all
all: $(INSTALL_DIR)/barebone_blink_pru0.bin $(INSTALL_DIR)/barebone_blink_pru1.bin $(INSTALL_DIR)/pwm_generator.bin



$(INSTALL_DIR)/%.bin: %.elf
	$(PRU_HEX) -b -o $@ $< --quiet


%.elf: %.obj $(LINKER_CMD)
	$(PRU_ASM) -z $(LINKER_CMD) -o $@ $<


%.obj: %.asm
	$(PRU_ASM) $(ASM_FLAGS) -c $<

