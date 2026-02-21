.PHONY: esp32-build esp32-flash esp32-stop esp32-clean lab-test lab-run

esp32-build:
	cd esp32 && cargo build

esp32-flash:
	cd esp32 && cargo run

esp32-stop:
	espflash erase-flash -p /dev/ttyUSB0

esp32-clean:
	cd esp32 && cargo clean

lab-test:
	cd lab && go test ./...

lab-run:
	cd lab && go run .
