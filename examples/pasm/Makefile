
PRU_ASM = pasm
ASM_FLAGS = -b

INSTALL_DIR = ..

.PHONY: all
all: $(INSTALL_DIR)/barebone_blink_pru0.bin $(INSTALL_DIR)/barebone_blink_pru1.bin $(INSTALL_DIR)/pwm_generator.bin


$(INSTALL_DIR)/%.bin: %.pasm
	$(PRU_ASM) $(ASM_FLAGS) $< $(INSTALL_DIR)/$*

