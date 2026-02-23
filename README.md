# GDML Studio

> **Early-stage project** — This is a fresh prototype under active development. It likely does not fully work yet. Expect rough edges, missing features, and bugs. Contributions and bug reports are welcome.

A lightweight desktop tool for viewing [GDML](https://gdml.web.cern.ch/GDML/) (Geometry Description Markup Language) detector geometry files. GDML is the standard geometry format used by [Geant4](https://geant4.web.cern.ch/) and other particle-physics simulation frameworks. GDML Studio lets you quickly inspect these geometries without launching a full Geant4 session — it parses the GDML XML, evaluates expressions and units, tessellates solids into triangle meshes, and renders the 3D scene in the browser.

<p align="center">
  <img src="screen_example.png" alt="GDML Studio screenshot" width="700">
</p>

## Architecture

| Layer | Tech | Path |
|-------|------|------|
| Backend | Rust / Axum | `backend/` |
| Frontend | React / Three.js (via React Three Fiber) | `frontend/` |

The backend exposes a REST API that the frontend consumes. Communication is JSON over HTTP.

## Prerequisites

- **Rust** stable toolchain — [install via rustup](https://rustup.rs/)
- **Node.js** v18+ and **npm** — [install from nodejs.org](https://nodejs.org/)

## Quick Start

### 1. Clone the repository

```bash
git clone git@github.com:davsar89/GDML_Studio.git
cd GDML_Studio
```

### 2. One-command start (recommended)

Scripts that build the backend, run tests, start both servers, and open the browser:

```bash
# Linux / macOS
./run.sh

# Windows
run.bat
```

### 3. Manual start

**Backend** (terminal 1):

```bash
cd backend
cargo run --release
```

The backend compiles and starts an HTTP server on `http://127.0.0.1:4001`.
On the first run, Cargo will download and compile all dependencies (this may take a minute or two).

**Frontend** (terminal 2):

```bash
cd frontend
npm install      # only needed on first run
npm run dev
```

Vite will print a local URL (typically `http://localhost:5173`).

### 4. Use the application

Open the Vite URL in your browser, click **Open File**, and select a `.gdml` file (e.g. one of the sample files below).

### Material Editor

The **Materials** tab in the left panel lists all materials and elements defined in the loaded GDML file. Select a material to edit its properties:

- **Density** — edit the numeric value and choose a unit (g/cm3, kg/m3, mg/cm3)
- **Formula** — set or clear the chemical formula
- **Z** — set or clear the atomic number (for simple materials)
- **Components** — add or remove element references with fraction or composite weights

Use the **NIST Material Lookup** button to search the built-in database of 309 Geant4 predefined materials (elemental, compound, HEP, space, and biochemical categories) and apply a NIST density to the selected material.

### Volume Material Assignment

Select a volume in the 3D scene or tree view to open the **Volume Detail** panel. Use the material dropdown to reassign which material a volume references.

### Save / Export

The toolbar provides two export options:

- **Save** — overwrites the original GDML file with the current state (materials, elements, volumes)
- **Save As** — saves to a new file path

## Sample Files

GDML files are included in `sample_data/` for quick testing:

| File | Size | Description |
|------|------|-------------|
| `sample_data/BgoDetModel_v2_00.gdml` | 158 KB | BGO detector model |
| `sample_data/NaiDetModelWithMLI_v2_00.gdml` | 167 KB | NaI detector model with MLI |
| `sample_data/fermi_simple_elements_satellite.gdml` | 7.7 KB | Fermi satellite simple geometry |
| `sample_data/test_all_features.gdml` | 4 KB | Test file exercising all solid types |

## Running Tests

```bash
# Backend tests
cd backend
cargo test

# Frontend type check
cd frontend
npx tsc --noEmit
```

## License

See individual files for details.
