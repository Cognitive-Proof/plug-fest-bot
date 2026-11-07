# Plug Fest Bot — Nom

Nom is a little robot that eats a file and tells you what its C2PA Content Credentials declare
about generative AI: **AI**, **Includes AI**, **No AI** or **Unknown**.

- `backend/`: Rust (axum + c2pa-rs). `POST /api/eat` reads the upload's manifest store and
  classifies it with the reference classifier from
  [c2pa-ai-examples](../c2pa-ai-examples) (`backend/src/classify.rs` is a verbatim copy).
- `src/`: React (Vite). Shows the verdict, **Tell me more** (via `c2pa-react-component` and
  `c2pa-react-ai-disclaimer`) and **How it works**.

The verdict describes what the signed provenance declares, not an analysis of the pixels. The
policy is documented in the c2pa-ai-examples README.

## Requirements

- `../c2pa-rs` and `../c2pa-ai-examples` checked out next to this repository.
- Rust 1.96 (pinned in `backend/rust-toolchain.toml`; rustup fetches it).
- Node.js 22 or later.

## Run in development

```sh
npm ci
npm run dev:api   # backend on http://localhost:8080
npm run dev       # frontend; /api is proxied to the backend
```

## Run in production

```sh
npm run build
cargo run --release --manifest-path backend/Cargo.toml
```

The backend serves the built frontend from `dist/` at http://localhost:8080 (set `PORT` to change it).

## Test

```sh
cargo test --release --manifest-path backend/Cargo.toml
```

This checks every file in `../c2pa-ai-examples/labels.json` gets its expected verdict.

## Privacy

Uploads are read in memory and never written to disk. The server never fetches remote manifests.
Trust uses the c2pa-ai-examples test trust anchors (`backend/settings/`).
