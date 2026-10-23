import type { DocumentSummary, MeshData, SceneNode, DefineValue, VolumeInfo, MaterialInfo, ElementInfo, NistMaterial } from '../store/types';
import { useAppStore } from '../store';

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(url, init);
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    throw new Error(`Could not reach the backend (${detail}). Is it running?`);
  }
  if (!res.ok) {
    // The error body may be non-JSON or not an object; access `.error` defensively.
    const body: unknown = await res.json().catch(() => null);
    const apiError =
      body && typeof body === 'object' && typeof (body as { error?: unknown }).error === 'string'
        ? (body as { error: string }).error
        : null;
    throw new Error(apiError ?? `HTTP ${res.status} ${res.statusText}`.trim());
  }
  return res.json() as Promise<T>;
}

/**
 * The main file references others via <file name=".."> but is not itself
 * referenced. Falls back to the file with the most references. References are
 * compared by basename, as uploads carry no directories.
 */
function detectMainFile(files: Record<string, string>): string {
  const refs: Record<string, string[]> = {};
  for (const [name, content] of Object.entries(files)) {
    refs[name] = Array.from(content.matchAll(/<file\s[^>]*name\s*=\s*(["'])(.*?)\1/g), (m) => m[2].split(/[\\/]/).pop()!);
  }
  const referenced = new Set(Object.values(refs).flat());
  const names = Object.keys(files);
  return names.find((name) => refs[name].length > 0 && !referenced.has(name))
    ?? names.sort((a, b) => refs[b].length - refs[a].length)[0];
}

function send<T>(method: 'POST' | 'PUT', url: string, body: unknown): Promise<T> {
  return fetchJson<T>(url, { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
}

/** Upload one file or a modular set; the backend keeps the result as the open document. */
export async function uploadFiles(files: Record<string, string>) {
  return send<DocumentSummary>('POST', '/api/files/upload-multi', { files, main_file: detectMainFile(files) });
}

export async function getSummary() {
  return fetchJson<DocumentSummary>('/api/document/summary');
}

export async function getMeshes() {
  return fetchJson<{
    meshes: Record<string, MeshData>;
    scene_graph: SceneNode;
    warnings: string[];
  }>('/api/document/meshes');
}

/**
 * Scene graph only. Editing a material never re-tessellates, so use this rather
 * than `getMeshes` after an edit: it avoids re-downloading every vertex in the
 * document and, because `meshes` is not replaced, avoids rebuilding every
 * BufferGeometry and re-uploading the scene to the GPU.
 */
export async function getScene() {
  return fetchJson<{ scene_graph: SceneNode; warnings: string[] }>('/api/document/scene');
}

export async function getDefines() {
  return fetchJson<{ defines: DefineValue[] }>('/api/document/defines');
}

export async function getMaterials() {
  return fetchJson<{ elements: ElementInfo[]; materials: MaterialInfo[] }>(
    '/api/document/materials',
  );
}

export async function getStructure() {
  return fetchJson<{ volumes: VolumeInfo[]; world_ref: string }>(
    '/api/document/structure',
  );
}

// ─── NIST Materials ─────────────────────────────────────────────────────────

export async function getNistMaterials(search?: string, category?: string) {
  const params = new URLSearchParams();
  if (search) params.set('search', search);
  if (category) params.set('category', category);
  return fetchJson<{ materials: NistMaterial[] }>(
    `/api/nist/materials?${params.toString()}`,
  );
}

export async function getNistMaterial(name: string) {
  return fetchJson<{ material: NistMaterial }>(`/api/nist/material?${new URLSearchParams({ name })}`);
}

/**
 * A request that edits the document. Flags unsaved edits once it succeeds.
 *
 * Tied to the endpoint rather than inferred from the refresh that follows:
 * "a mutating request succeeded" is exactly what dirty means, and one added
 * later cannot quietly skip it. A rejected edit changed nothing.
 */
async function mutate(method: 'POST' | 'PUT', url: string, body: unknown) {
  const result = await send<{ ok: boolean }>(method, url, body);
  useAppStore.getState().markDirty();
  return result;
}

// ─── Material CRUD ──────────────────────────────────────────────────────────

export async function updateMaterial(name: string, material: MaterialInfo, elements: ElementInfo[] = []) {
  return mutate('PUT', '/api/document/materials/update', { name, material, elements });
}

export async function addMaterial(material: MaterialInfo, elements: ElementInfo[] = []) {
  return mutate('POST', '/api/document/materials/add', { material, elements });
}

export async function deleteMaterial(name: string) {
  return mutate('POST', '/api/document/materials/delete', { name });
}

// ─── Element CRUD ───────────────────────────────────────────────────────────

export async function addElement(element: ElementInfo) {
  return mutate('POST', '/api/document/elements/add', { element });
}

export async function deleteElement(name: string) {
  return mutate('POST', '/api/document/elements/delete', { name });
}

// ─── Volume material ref ────────────────────────────────────────────────────

export async function updateVolumeMaterialRef(volumeName: string, materialRef: string) {
  return mutate('PUT', '/api/document/structure/material-ref', { volume_name: volumeName, material_ref: materialRef });
}

// ─── Export ─────────────────────────────────────────────────────────────────

export async function exportGdml() {
  return fetchJson<{ gdml: string; filename: string }>(
    '/api/document/export',
    { method: 'POST' },
  );
}
