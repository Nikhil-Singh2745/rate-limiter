# Rate Limiter (Rust + Actix Web + Redis)

A small HTTP service that answers “am I allowed to make this request now?” using a token-bucket rate limiter backed by Redis.

## Features

- HTTP API for rate-limit checks
- Token-bucket algorithm with time-based refill
- Redis-backed storage with atomic updates via Lua
- Health endpoint to verify Redis connectivity
- Structured logging via `tracing`

## How It Works

1. Each client has a “bucket” of tokens in Redis.
2. Tokens represent how many requests can be made at that moment.
3. Tokens refill over time at a rate derived from `limit` (requests per minute).
4. If there is at least one token, the request is allowed and one token is consumed.
5. If not, the service returns how long to wait (`retry_after_ms`) until the next token is available.

Client identity is taken from the `X-API-Key` header if provided, otherwise the client IP address.

## API

### POST `/check`

Checks if a request is allowed right now and returns remaining tokens and retry time.

Request body:
```json
{
  "limit": 60,
  "burst": 100
}
```

- `limit` (required): requests per minute.
- `burst` (optional): maximum tokens the bucket can hold. If omitted, `burst` defaults to `limit`.

Response (200 OK if allowed, 429 Too Many Requests if blocked):
```json
{
  "allowed": true,
  "remaining": 99,
  "retry_after_ms": 0
}
```

- `allowed`: whether the request is allowed.
- `remaining`: tokens remaining after this check.
- `retry_after_ms`: if blocked, milliseconds to wait before retrying.

### GET `/health`

Returns Redis connectivity status:
```json
{ "status": "ok" }
```
or
```json
{ "status": "unhealthy" }
```

## Quick Start

### Prerequisites
- Rust (stable)
- Redis (local or remote)

Start Redis locally (Docker):
```bash
docker run --rm -p 6379:6379 redis:7-alpine
```

### Configuration

Environment variables:
- `REDIS_URL` (default: `redis://127.0.0.1:6379`)
- `HOST` (default: `127.0.0.1`)
- `PORT` (default: `8080`)

### Run

```bash
cargo run
```

The server starts at `http://HOST:PORT`, e.g. `http://127.0.0.1:8080`.

### Try It

Allow a request at up to 60 rpm with a burst of 100 tokens:

```bash
curl -X POST http://127.0.0.1:8080/check \
  -H "Content-Type: application/json" \
  -H "X-API-Key: my-client-key" \
  -d '{"limit":60,"burst":100}'
```

Health check:

```bash
curl http://127.0.0.1:8080/health
```

## Implementation Notes

- The token bucket is stored in Redis and updated atomically using a Lua script.
- Keys are set to expire (currently `EXPIRE 120` seconds) to avoid stale data.
- Tokens are computed with fractional values for smoother refill; the returned `remaining` is floored to an integer.

## Project Structure

- `src/main.rs` — Server bootstrap, environment config, route registration
- `src/handlers.rs` — HTTP handlers for `/check` and `/health`
- `src/rate_limiter.rs` — Rate limiter logic, Redis integration, Lua script, unit tests
- `Cargo.toml` — Dependencies and crate metadata

## Testing

Unit tests for core token-bucket calculations are included:
```bash
cargo test
```

## License
This project is licensed under the MIT License.

