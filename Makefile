.PHONY: esp32-build esp32-flash esp32-stop esp32-clean lab-up lab-down lab-logs

esp32-build:
	. ~/export-esp.sh && cd esp32 && cargo +esp build-devkit

esp32-flash:
	. ~/export-esp.sh && cd esp32 && cargo +esp run-devkit

esp32-stop:
	espflash erase-flash -p /dev/ttyUSB0

lab-up:
	docker compose up -d --build

lab-down:
	docker compose down

lab-logs:
	docker compose logs -f
