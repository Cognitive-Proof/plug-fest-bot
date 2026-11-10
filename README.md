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

## C2PA signals

"Tell me more" also shows the C2PA Conformance Task Force's signals rubric for each signed step
(`src/signals/`). `/api/eat` returns the manifest store as crJSON (`crjson`), and the browser
evaluates the rubric with `src/signals/evaluator.js`, a JavaScript port of the signals part of the
Task Force's Python evaluator (`asset-rubrics/c2pa_signals_rubric_evaluator.py`), using
`@adobe/json-formula`.

The rubric is copied from a conformance checkout into `src/signals/asset-rubric-signals-local.json`,
which records the commit it came from:

```sh
npm run sync:signals -- ../conformance     # copy the rubric at that checkout's commit
npm run check:signals                       # JS output must equal the Python goldens
```

`check:signals` compares the port with every `*.signals.json` next to a crJSON file in the given
directories (default `../conformance/asset-rubrics/test`).

## Uploads and feedback

When `UPLOAD_BUCKET` and `GOOGLE_CLOUD_PROJECT` are set (Cloud Run sets them in `cloudbuild.yaml`),
the backend keeps every upload so we can find the files Plug-Fest Bot gets wrong:

- The file goes to `gs://plug-fest-bot-uploads/uploads/<id>/<name>` (private bucket).
- Firestore `uploads/<id>` records `fileName`, `size`, `contentType`, `sha256`, `uploadedAt`,
  `storagePath`, `verdict`, `reason` and `sample` (one of the site's own sample files).
- The Right / Wrong buttons call `POST /api/feedback`, which adds
  `feedback: { correct, expected, submittedAt }` to that document (`expected` only when wrong).

Without those variables nothing is kept and the buttons are hidden. To record from your machine:

```sh
gcloud auth application-default login
UPLOAD_BUCKET=plug-fest-bot-uploads GOOGLE_CLOUD_PROJECT=plug-fest-bot npm run dev:api
```

## Privacy

Uploads are never written to the server's disk; when recording is on they are kept in the bucket
above. When a file only links to its manifest (a remote manifest, e.g. Adobe's cloud Content
Credentials), the server fetches it once with limits: HTTPS only, public addresses only (the
connection is pinned to the checked address), no redirects, 5 MB and 10 seconds at most
(`backend/src/remote.rs`). The URL is recorded as `remoteManifest`. Trust uses the c2pa-ai-examples test trust
anchors plus the official C2PA signer and time-stamping trust lists from
[c2pa-org/conformance-public](https://github.com/c2pa-org/conformance-public/tree/main/trust-list)
(`backend/settings/`); set `C2PA_SETTINGS` to use another settings file.
