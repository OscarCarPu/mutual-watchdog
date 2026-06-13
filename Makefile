.PHONY: esp32-build-dev esp32-build-prod esp32-build esp32-flash-dev esp32-flash-prod esp32-stop-dev esp32-stop-prod esp32-test consumer-up consumer-down consumer-logs

esp32-build-dev:
	. ~/export-esp.sh && cd esp32 && cargo +esp build-devkit

esp32-build-prod:
	. ~/export-esp.sh && cd esp32 && cargo +esp build-xiao

esp32-build: esp32-build-dev esp32-build-prod

esp32-flash-dev:
	. ~/export-esp.sh && cd esp32 && cargo +esp run-devkit

esp32-flash-prod:
	. ~/export-esp.sh && cd esp32 && cargo +esp run-xiao

esp32-stop-dev:
	espflash erase-flash -p /dev/ttyUSB0

esp32-stop-prod:
	espflash erase-flash -p /dev/ttyACM0

esp32-test:
	cd esp32 && cargo test-logic

consumer-up:
	docker compose up -d --build

consumer-down:
	docker compose down

consumer-logs:
	docker compose logs -f
