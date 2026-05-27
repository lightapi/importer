# importer

Rust replacement for the Java `event-importer` CLI.

It supports the two portal migration workflows:

1. Import a JSON array of CloudEvents into `event_store_t`,
   `outbox_message_t`, and pending `notification_t`.
2. Convert a portal snapshot JSON file into CreatedEvents that can be imported
   by the same import path.

## Build

```bash
cargo build --release
```

The binary is created at `target/release/importer`.

## Release

Create a GitHub release and upload Linux plus macOS Apple Silicon archives:

```bash
./release.sh v0.1.0
```

The script creates:

- `dist/importer-v0.1.0-linux-x86_64.tar.gz`
- `dist/importer-v0.1.0-macos-aarch64.tar.gz`
- `dist/SHA256SUMS`

Requirements:

- `gh` authenticated with permission to create/upload releases.
- `rustup` targets for `x86_64-unknown-linux-gnu` and
  `aarch64-apple-darwin`. The script installs missing targets when `rustup` is
  available.
- For cross-building macOS Apple Silicon from Linux, install `zig` and
  `cargo-zigbuild`, or run the release script on macOS. Without those tools,
  publish Linux only with `BUILD_MACOS=0 ./release.sh v0.1.0`.

On Linux, install the Rust cross-build helper with:

```bash
cargo install cargo-zigbuild
```

Install `zig` from your OS package manager or from <https://ziglang.org/download/>.

Useful release options:

```bash
BUILD_MACOS=0 ./release.sh v0.1.0
BUILD_LINUX=0 ./release.sh v0.1.0
UPLOAD=0 ./release.sh v0.1.0
ALLOW_DIRTY=1 ./release.sh v0.1.0
```

## Configuration

Import mode writes to PostgreSQL and requires `DATABASE_URL`:

```bash
export DATABASE_URL=postgres://postgres:secret@localhost:5432/configserver
```

Optional:

```bash
export IMPORTER_DB_MAX_CONNECTIONS=3
export RUST_LOG=info
```

## Import Events

```bash
importer import --filename events.json
```

Legacy Java-compatible form:

```bash
importer --filename events.json
```

Useful options:

```bash
importer import \
  --filename events.json \
  --replacement '[{"field":"hostId","from":"OLD_HOST_UUID","to":"NEW_HOST_UUID"}]' \
  --batch-size 500
```

Use `--dry-run` to parse, mutate, and validate without database access. Dry-run
does not simulate target-database skips, so `skippedExistingTarget` remains `0`.

`--filename -` reads from stdin.

## Convert Snapshot

```bash
importer convert \
  --filename snapshot.json \
  --target-host-id 01964b05-552a-7c4b-9184-6857e7f3dc5f \
  --admin-user-id 01964b05-5532-7c79-8cde-191dcbd421b8 \
  --output events.json
```

Legacy Java-compatible form:

```bash
importer --convert \
  --filename snapshot.json \
  --targetHostId 01964b05-552a-7c4b-9184-6857e7f3dc5f \
  --adminUserId 01964b05-5532-7c79-8cde-191dcbd421b8 \
  --output events.json
```

Convert and import without writing an intermediate file:

```bash
importer convert \
  --filename snapshot.json \
  --target-host-id 01964b05-552a-7c4b-9184-6857e7f3dc5f \
  --admin-user-id 01964b05-5532-7c79-8cde-191dcbd421b8 \
  --output - \
  | importer import --filename - --batch-size 500
```

Snapshot conversion defaults to the embedded portal table dependency graph. Use
`--schema-source database` to validate ordering against live PostgreSQL
metadata.

## Mutation Rules

Replacement rules may use the Java aliases:

```json
[
  {"field":"hostId","from":"OLD_HOST_UUID","to":"NEW_HOST_UUID"}
]
```

or:

```json
[
  {"fieldName":"hostId","fromValue":"OLD_HOST_UUID","toValue":"NEW_HOST_UUID"}
]
```

Enrichment rules support `generateUUID`, `mapGenerate`, and the README legacy
alias `mapAndGenerate`.
