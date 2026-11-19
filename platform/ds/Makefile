BLOCKSDS ?= /opt/blocksds/core
WONDERFUL_TOOLCHAIN ?= /opt/wonderful

TARGET ?= ../../target
OUT ?= $(TARGET)/ds
RUST_LIB := $(TARGET)/armv5te-none-eabi/nds/librainy_day_ds.a

ARM_CC := $(WONDERFUL_TOOLCHAIN)/toolchain/gcc-arm-none-eabi/bin/arm-none-eabi-gcc
ARM7 := $(BLOCKSDS)/sys/arm7/main_core/arm7_dswifi.elf
LIBNDS := $(BLOCKSDS)/libs/libnds
DSWIFI := $(BLOCKSDS)/libs/dswifi
NDSTOOL := $(BLOCKSDS)/tools/ndstool/ndstool
ARM9_FLAGS := -mthumb -mcpu=arm946e-s+nofp -specs=$(BLOCKSDS)/sys/crts/ds_arm9.specs

.PHONY: all
all: $(OUT)/a-rainy-day.nds

$(OUT)/a-rainy-day.nds: $(OUT)/arm9.elf $(ARM7) icon.png Makefile
	$(NDSTOOL) -c "$@" -9 "$<" -7 "$(ARM7)" -uc 0 \
		-g '####' 00 "a rainy day" -b icon.png "a rainy day"

$(OUT)/arm9.elf: main.c voice.c voice.h $(RUST_LIB) Makefile
	mkdir -p "$(OUT)"
	$(ARM_CC) $(ARM9_FLAGS) -D__NDS__ -D__BLOCKSDS__ -DARM9 \
		-std=gnu17 -O2 -Wall -Wextra -ffunction-sections -fdata-sections \
		-I"$(LIBNDS)/include" -I"$(DSWIFI)/include" \
		-L"$(LIBNDS)/lib" -L"$(DSWIFI)/lib" -Wl,--gc-sections \
		-o "$@" main.c voice.c -Wl,--start-group "$(RUST_LIB)" -ldswifi9 -lnds9 -lc -Wl,--end-group
