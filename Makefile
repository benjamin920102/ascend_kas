CANN_ARCH ?= dav-2201
BATCH ?= 4096
VEC_GRID ?= 40
CUBE_GRID ?= 20
CUBE_SINGLE_N ?= 224
OUT := build

.PHONY: all kernel rust test clean
all: kernel rust

kernel:
	mkdir -p $(OUT)
	cd kernel && rm -rf build && mkdir build && cd build && \
	  cmake -DCMAKE_ASC_RUN_MODE=npu \
	        -DCMAKE_ASC_ARCHITECTURES=$(CANN_ARCH) \
	        -DASCEND_KAS_BATCH=$(BATCH) \
	        -DASCEND_KAS_VEC_GRID=$(VEC_GRID) \
	        -DASCEND_KAS_CUBE_GRID=$(CUBE_GRID) \
	        -DASCEND_KAS_CUBE_SINGLE_N=$(CUBE_SINGLE_N) \
	        -DCMAKE_MODULE_PATH="$$ASC_MODULES" .. && \
	  $(MAKE) -j
	cp kernel/build/libascend_kas.so $(OUT)/

rust:
	cargo build --release
	mkdir -p $(OUT)
	cp target/release/ascend_kas $(OUT)/

test:
	cargo test

clean:
	rm -rf $(OUT) kernel/build target
