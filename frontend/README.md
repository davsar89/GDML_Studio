# GDML Studio — Frontend

React + TypeScript + Vite, rendering GDML geometry with Three.js via
[React Three Fiber](https://r3f.docs.pmnd.rs/). State lives in a single
[Zustand](https://zustand.docs.pmnd.rs/) store.

See the [root README](../README.md) for how to run the whole application. This
file covers the frontend only.

## Development

```bash
npm install
npm run dev
```

Vite serves on `http://localhost:5173` and proxies `/api` to the Rust backend on
`http://127.0.0.1:4001` (see [`vite.config.ts`](vite.config.ts)), so the backend
must be running for anything to load.

## Checks

```bash
npm run lint
npx tsc -b
npm run build
```

**Use `tsc -b`, not `tsc --noEmit`.** `tsconfig.json` is a solution-style config
(`"files": []` plus project references), so a non-build invocation resolves zero
input files, checks nothing, and exits 0 no matter how many type errors exist.

## Layout

| Path | Contents |
|------|----------|
| `src/api/client.ts` | Typed wrappers over the backend REST API |
| `src/store/` | Zustand store and the TypeScript mirrors of the backend's JSON shapes |
| `src/components/Viewport/` | R3F canvas, scene graph, geometry cache, measurement tools |
| `src/components/TreePanel/` | Volume tree, volume detail, material and element editors |
| `src/utils/` | NIST material import, post-edit refresh helpers |

### A note on `src/store/types.ts`

`MaterialInfo` and `ElementInfo` are POSTed back to the backend, which replaces
the stored item **wholesale**. Every field the backend models must appear in
these interfaces, and edit paths must spread the original object rather than
rebuild it field by field — any field omitted from the payload is silently
dropped from the document and from the exported GDML.
