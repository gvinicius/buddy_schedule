.PHONY: help db-up db-down db-reset test lint fmt run

help:
	@echo "Available commands:"
	@echo "  make db-up      - Start PostgreSQL database with podman"
	@echo "  make db-down    - Stop PostgreSQL database"
	@echo "  make db-reset   - Stop and remove database (WARNING: deletes all data)"
	@echo "  make test       - Run tests"
	@echo "  make lint       - Run clippy linter"
	@echo "  make fmt        - Format code"
	@echo "  make run        - Run the application"

db-up:
	@echo "Starting PostgreSQL database with podman..."
	@podman run -d \
		--name buddy_schedule_db \
		-e POSTGRES_USER=postgres \
		-e POSTGRES_PASSWORD=postgres \
		-e POSTGRES_DB=buddy_schedule \
		-p 5432:5432 \
		-v buddy_schedule_postgres_data:/var/lib/postgresql/data \
		postgres:15 || \
	podman start buddy_schedule_db
	@echo "Waiting for database to be ready..."
	@sleep 3
	@echo "Database is ready!"

db-down:
	@echo "Stopping PostgreSQL database..."
	@podman stop buddy_schedule_db || true
	@echo "Database stopped."

db-reset:
	@echo "WARNING: This will delete all database data!"
	@podman stop buddy_schedule_db || true
	@podman rm buddy_schedule_db || true
	@podman volume rm buddy_schedule_postgres_data || true
	@echo "Database removed."

test:
	@cargo test

lint:
	@cargo clippy --all-targets --all-features -- -D warnings

fmt:
	@cargo fmt --all

run:
	@cargo run

build-web:
	@echo "Building WASM frontend..."
	@cd web-frontend && ./build.sh

dev: build-web run
