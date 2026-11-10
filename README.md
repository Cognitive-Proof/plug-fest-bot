# Plug Fest Bot — Nom

Nom is a little robot that eats a file and tells you what its C2PA Content Credentials declare
about generative AI: **AI**, **Includes AI**, **No AI** or **Unknown**.

- `backend/`: Rust (axum + c2pa-rs). `POST /api/eat` reads the upload's manifest store and
  classifies it with the reference classifier from
  [c2pa-ai-examples](../c2pa-ai-examples) (`backend/src/classify.rs` is a verbatim copy).
- `backend/` also serves a public API (`/api/v1/…`) and an MCP server (`/mcp`); see below.
- `src/`: React (Vite). Shows the verdict, **Tell me more** (via `c2pa-react-component` and
  `c2pa-react-ai-disclaimer`) and **How it works**.

The verdict describes what the signed provenance declares, not an analysis of the pixels. The
policy is documented in the c2pa-ai-examples README. It reads an omitted `allActionsIncluded` by
the spec version the claim declares: optional before C2PA 2.5, so an omission counts as a
complete history for an earlier version (or none), and required from 2.5.

## Requirements

- `../c2pa-rs` and `../c2pa-ai-examples` checked out next to this repository. The deployed bot
  builds the c2pa-rs commit pinned in the `Dockerfile` (`C2PA_RS_REV`); check that commit out to
  get the same verdicts locally.
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

## Public API

For scripts. Nothing sent here is kept, whatever the recording settings below.

```sh
curl -F file=@photo.jpg https://<host>/api/v1/classify
curl -H 'Content-Type: application/json' -d '{"url":"https://example.com/photo.jpg"}' https://<host>/api/v1/classify-url
```

Both answer with `verdict`, `reason`, `fileName`, `size`, `remoteManifest` and `manifest` (the
active manifest's generator, signer and validation state). `?detail=true` adds `outcome` and
`crjson`, as the website gets them. `classify-url` downloads the file with the same limits as a
remote manifest (HTTPS, public addresses, no redirects) but up to 100 MB and 60 seconds.

## MCP server

`https://<host>/mcp` is a remote MCP server (Streamable HTTP, stateless, no sign-in) with two
tools:

- `classify_url`: give it a file's HTTPS URL; the server downloads and classifies it.
- `classify_file`: the file inline as base64, up to 20 MB.

Add it to an MCP client as a remote (HTTP) server with that URL, for example
`claude mcp add --transport http plug-fest-bot https://<host>/mcp`. Like the API, it keeps nothing.

## Rate limit

`/api/*` and `/mcp` allow 60 requests a minute from one address on each instance (Cloud Run runs
at most 3). Set `RATE_LIMIT_PER_MINUTE` to change it, or `0` to turn it off (for bulk runs on
your own machine).

## Uploads and feedback

When `UPLOAD_BUCKET` and `GOOGLE_CLOUD_PROJECT` are set (Cloud Run sets them in `cloudbuild.yaml`),
the backend keeps every upload so we can find the files Plug-Fest Bot gets wrong:

- The file goes to `gs://plug-fest-bot-uploads/uploads/<id>/<name>` (private bucket).
- Firestore `uploads/<id>` records `fileName`, `size`, `contentType`, `sha256`, `uploadedAt`,
  `storagePath`, `verdict`, `reason` and `sample` (one of the site's own sample files).
- The Right / Wrong buttons call `POST /api/feedback`, which adds
  `feedback: { correct, expected, submittedAt }` to that document (`expected` only when wrong).

Without those variables, or with `NO_STORAGE=true`, nothing is kept: no bucket or Firestore is
touched, the Right / Wrong buttons are hidden and How it works says files aren't kept
(`GET /api/info` tells the frontend). To turn recording off on Cloud Run, add `NO_STORAGE=true`
to `--set-env-vars` in `cloudbuild.yaml`. To record from your machine:

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
