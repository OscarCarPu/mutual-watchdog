.PHONY: esp32-build esp32-flash esp32-monitor esp32-clean

esp32-build:
	cd esp32 && cargo build

esp32-flash:
	cd esp32 && cargo run

esp32-monitor:
	espflash monitor

esp32-clean:
	cd esp32 && cargo clean
