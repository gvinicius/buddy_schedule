# Buddy Schedule API

A Rust-based schedule management API for managing shifts, schedules, and rotations.

## Features

- **JWT Authentication**: Secure token-based authentication
- **Schedules**: Create and manage schedules for subjects (person/family/pet/etc)
- **Roles**: 
  - **superadmin**: Global admin (first registered user becomes superadmin automatically)
  - **admin/user**: Per-schedule membership roles
- **Shifts**: Create and list shifts (morning/afternoon/night/sleep), assign to users
- **Comments**: Add rotation notes to shifts
- **Rotation Templates**: Store JSON templates and apply them to specific week start dates

## Prerequisites

- Rust (latest stable version)
- Podman or Docker (for database)
- PostgreSQL client tools (optional, for direct database access)

## Setup

### 1. Clone and navigate to the project

```bash
cd buddy_schedule
```

### 2. Start the database with Podman

```bash
# Using podman-compose (if available)
podman-compose -f podman-compose.yml up -d

# Or using podman directly
podman run -d \
  --name buddy_schedule_db \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=buddy_schedule \
  -p 5432:5432 \
  -v postgres_data:/var/lib/postgresql/data \
  postgres:15
```

### 3. Configure environment variables

```bash
cp .env.example .env
# Edit .env and set your DATABASE_URL and JWT_SECRET
```

### 4. Run database migrations

The migrations will run automatically when you start the application, or you can run them manually:

```bash
# Make sure DATABASE_URL is set in your environment
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/buddy_schedule
sqlx migrate run
```

### 5. Run the application

```bash
cargo run
```

The API will be available at `http://localhost:8080`

## API Quick Start

### Register a user (first user becomes superadmin)

```bash
curl -X POST http://localhost:8080/api/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"password123"}'
```

### Login

```bash
TOKEN=$(curl -sS -X POST http://localhost:8080/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"password123"}' | jq -r .token)
```

### Create a schedule

```bash
curl -X POST http://localhost:8080/api/schedules \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"name":"Care Rota","subject_type":"pet","subject_name":"Puppy"}'
```

## Development

### Running tests

```bash
# Make sure you have a test database running
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/buddy_schedule_test
cargo test
```

### Linting and formatting

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings
```

## Rotation Template Format

Templates are stored as JSON and can be applied to a week start date:

```json
{
  "slots": [
    { "dow": 0, "period": "morning", "start": "08:00", "end": "12:00" },
    { "dow": 0, "period": "afternoon", "start": "12:00", "end": "18:00" },
    { "dow": 1, "period": "night", "start": "18:00", "end": "22:00" }
  ]
}
```

Where:
- `dow`: Day of week (0=Monday, 6=Sunday)
- `period`: One of "morning", "afternoon", "night", "sleep"
- `start`/`end`: Time in HH:MM format

## CI/CD

GitHub Actions workflows are configured to:
- Run tests on pull requests and pushes to main/master
- Check code formatting and run clippy linting

## Database Management

### Stop the database

```bash
podman stop buddy_schedule_db
# or
podman-compose -f podman-compose.yml down
```

### Remove the database (WARNING: deletes all data)

```bash
podman rm -f buddy_schedule_db
podman volume rm postgres_data
# or
podman-compose -f podman-compose.yml down -v
```

## License

[Add your license here]
