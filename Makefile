.PHONY: esp32-build-dev esp32-build-prod esp32-flash-dev esp32-flash-prod esp32-stop-dev esp32-stop-prod lab-up lab-down lab-logs

esp32-build-dev:
	. ~/export-esp.sh && cd esp32 && cargo +esp build-devkit

esp32-build-prod:
	. ~/export-esp.sh && cd esp32 && cargo +esp build-supermini

esp32-flash-dev:
	. ~/export-esp.sh && cd esp32 && cargo +esp run-devkit

esp32-flash-prod:
	. ~/export-esp.sh && cd esp32 && cargo +esp run-supermini

esp32-stop-dev:
	espflash erase-flash -p /dev/ttyUSB0

esp32-stop-prod:
	espflash erase-flash -p /dev/ttyACM0

lab-up:
	docker compose up -d --build

lab-down:
	docker compose down

lab-logs:
	docker compose logs -f
