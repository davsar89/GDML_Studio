import * as api from '../api/client';
import { useAppStore } from '../store';
import type { DocumentSummary, ImportOptions, ImportProgress, NistMaterial, PartMaterial } from '../store/types';
import { loadDocument } from './refresh';
import { prepareNistMaterial } from './nistImport';

/** The files Open imports rather than parses as GDML (backend `cad::format_of`). */
export const isCadFile = (name: string) => /\.(stl|step|stp)$/i.test(name);

/** The last import's options, which the next import starts from; no per-part choices. */
const STORAGE_KEY = 'gdml-studio:cad-import';

type Remembered = Omit<ImportOptions, 'part_material' | 'overrides'> & { part_material: string | null };

function remembered(): Remembered {
  let stored: Partial<Record<keyof Remembered, unknown>> = {};
  try {
    stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') ?? {};
  } catch {
    // No storage (a private window) or nothing valid in it: the defaults.
  }
  const positive = (x: unknown, otherwise: number) => typeof x === 'number' && Number.isFinite(x) && x > 0 ? x : otherwise;
  return {
    stl_unit_mm: positive(stored.stl_unit_mm, 1),
    volume_target: positive(stored.volume_target, 1e-3),
    hierarchy: stored.hierarchy === 'flatten' ? 'flatten' : 'keep',
    part_material: typeof stored.part_material === 'string' ? stored.part_material : null,
  };
}

/** The phases of an import (backend `cad` and its STEP worker), as the toolbar names them. */
const PHASES: Record<string, string> = {
  read: 'reading the file', mesh: 'meshing and recognizing solids', interfere: 'checking interferences and contacts',
  validate: 'validating meshes',
  remesh: 'remeshing', check: 'checking for crossing meshes', build: 'building the document',
};

export const progressText = ({ phase, done, total }: ImportProgress) =>
  phase in PHASES ? `Importing: ${PHASES[phase]}${total > 1 ? ` ${done}/${total}` : ''}` : 'Importing…';

const PROGRESS_POLL_MS = 500;

/** Run an import request, keeping the store's `importProgress` up to date until it settles. */
async function withProgress(request: () => Promise<DocumentSummary>): Promise<DocumentSummary> {
  useAppStore.setState({ importProgress: { phase: '', done: 0, total: 0 } });
  let running = true;
  const poll = setInterval(() => api.getImportProgress().then((progress) => {
    if (running && progress) useAppStore.setState({ importProgress: progress });
  }, () => {}), PROGRESS_POLL_MS);
  try {
    return await request();
  } finally {
    running = false;
    clearInterval(poll);
    useAppStore.setState({ importProgress: null });
  }
}

/** Whether the user cancelled the running import. */
let cancelled = false;

/** Stop the running import; the open document stays as it was. */
export function cancelImport(): void {
  cancelled = true;
  api.cancelImport().catch(() => {});
}

/** Load what an import request made, show its report and remember its options. */
async function loadImport(request: () => Promise<DocumentSummary>, what: string): Promise<void> {
  const before = useAppStore.getState().importReport;
  cancelled = false;
  await loadDocument(() => withProgress(request), what);
  const report = useAppStore.getState().importReport;
  if (!report || report === before) {
    // Failing is what a cancelled import does: no error to show.
    if (cancelled) useAppStore.getState().setError(null);
    return;
  }
  useAppStore.getState().setActiveTreeTab('import');
  const { stl_unit_mm, volume_target, hierarchy, part_material } = report.options;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      stl_unit_mm, volume_target, hierarchy, part_material: part_material?.material.name ?? null,
    } satisfies Remembered));
  } catch {
    // No storage (a private window): the next import starts from the defaults.
  }
}

/**
 * Import `file` as a new document with the last import's options. The caller
 * owns the document action. A remembered part material is looked up in the
 * NIST table, so one defined only in an earlier document is not carried over.
 */
export async function importCadFile(file: File): Promise<void> {
  const { part_material: name, ...last } = remembered();
  const nist = name === null ? null : await api.getNistMaterial(name).then((r) => r.material, () => null);
  const options: ImportOptions = {
    ...last,
    part_material: nist ? await prepareNistMaterial(nist, []) : null,
    overrides: {},
  };
  await loadImport(() => api.importCad(file, options), file.name);
}

/** A part material picked in the Import panel: a document material, or a NIST one to add. */
export interface MaterialChoice { name: string; nist?: NistMaterial }

/**
 * Rebuild the open imported document with new options, `choice` giving the
 * part material. The caller owns the document action.
 */
export async function reimport(options: Omit<ImportOptions, 'part_material'>, choice: MaterialChoice | null): Promise<void> {
  const { importReport: report, materials } = useAppStore.getState();
  if (!report) return;
  const current = report.options.part_material;
  let part_material: PartMaterial | null = null;
  if (choice?.name === current?.material.name) part_material = current;
  else if (choice?.nist) part_material = await prepareNistMaterial(choice.nist);
  else if (choice) {
    // The picker offers the document's materials and NIST ones it lacks.
    const material = materials.find((m) => m.name === choice.name);
    if (!material) throw new Error(`The document has no material '${choice.name}'.`);
    part_material = { material, elements: [] };
  }
  await loadImport(() => api.updateImport({ ...options, part_material }), report.filename);
}
