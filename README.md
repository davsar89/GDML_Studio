# GDML Studio

> **Early-stage project** — This is a fresh prototype under active development. It likely does not fully work yet. Expect rough edges, missing features, and bugs. Contributions and bug reports are welcome.

A lightweight desktop tool for viewing [GDML](https://gdml.web.cern.ch/GDML/) (Geometry Description Markup Language) detector geometry files. GDML is the standard geometry format used by [Geant4](https://geant4.web.cern.ch/) and other particle-physics simulation frameworks. GDML Studio lets you quickly inspect these geometries without launching a full Geant4 session — it parses the GDML XML, evaluates expressions and units, tessellates solids into triangle meshes, and renders the 3D scene in the browser.

## Architecture

| Layer | Tech | Path |
|-------|------|------|
| Backend | Rust / Axum | `backend/` |
| Frontend | React / Three.js (via React Three Fiber) | `frontend/` |

The backend exposes a REST API that the frontend consumes. Communication is JSON over HTTP.

## Prerequisites

- **Rust** (stable toolchain)
- **Node.js** (v18+) and **npm**

## Getting Started

```bash
# Backend
cd backend
cargo run --release

# Frontend (separate terminal)
cd frontend
npm install
npm run dev
```

Open the URL printed by Vite, click **Open File**, and provide the path to a `.gdml` file.

## Sample Files

A few GDML files are included in the repo root for quick testing:

- `sample_data/BgoDetModel_v2_00.gdml`
- `sample_data/NaiDetModelWithMLI_v2_00.gdml`
- `sample_data/fermi_simple_elements_satellite.gdml`

## Tests

```bash
cargo test        # backend integration tests
cd frontend && npx tsc --noEmit   # frontend type check
```

## License

See individual files for details.
