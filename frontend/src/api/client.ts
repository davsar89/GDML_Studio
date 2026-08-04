import type { DocumentSummary, MeshData, SceneNode, DefineValue, VolumeInfo, MaterialInfo, ElementInfo, NistMaterial } from '../store/types';

const BASE = '';

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${BASE}${url}`, init);
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

export async function uploadFile(filename: string, content: string) {
  return fetchJson<DocumentSummary>('/api/files/upload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ filename, content }),
  });
}

export async function uploadFiles(
  files: Record<string, string>,
  mainFile: string,
) {
  return fetchJson<DocumentSummary>('/api/files/upload-multi', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ files, main_file: mainFile }),
  });
}

export async function getMeshes() {
  return fetchJson<{
    meshes: Record<string, MeshData>;
    scene_graph: SceneNode;
    warnings?: string[];
  }>('/api/document/meshes');
}

/**
 * Scene graph only. Editing a material never re-tessellates, so use this rather
 * than `getMeshes` after an edit: it avoids re-downloading every vertex in the
 * document and, because `setMeshes` is not called, avoids rebuilding every
 * BufferGeometry and re-uploading the scene to the GPU.
 */
export async function getScene() {
  return fetchJson<{
    scene_graph: SceneNode;
    warnings?: string[];
  }>('/api/document/scene');
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
  const params = new URLSearchParams();
  params.set('name', name);
  return fetchJson<{ material: NistMaterial }>(
    `/api/nist/material?${params.toString()}`,
  );
}

// ─── Material CRUD ──────────────────────────────────────────────────────────

export async function updateMaterial(name: string, material: MaterialInfo) {
  return fetchJson<{ ok: boolean }>('/api/document/materials/update', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, material }),
  });
}

export async function addMaterial(material: MaterialInfo) {
  return fetchJson<{ ok: boolean }>('/api/document/materials/add', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ material }),
  });
}

export async function deleteMaterial(name: string) {
  return fetchJson<{ ok: boolean }>('/api/document/materials/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  });
}

// ─── Element CRUD ───────────────────────────────────────────────────────────

export async function addElement(element: ElementInfo) {
  return fetchJson<{ ok: boolean }>('/api/document/elements/add', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ element }),
  });
}

export async function deleteElement(name: string) {
  return fetchJson<{ ok: boolean }>('/api/document/elements/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  });
}

// ─── Volume material ref ────────────────────────────────────────────────────

export async function updateVolumeMaterialRef(
  volumeName: string,
  materialRef: string,
) {
  return fetchJson<{ ok: boolean }>('/api/document/structure/material-ref', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ volume_name: volumeName, material_ref: materialRef }),
  });
}

// ─── Export ─────────────────────────────────────────────────────────────────

export async function exportGdml() {
  return fetchJson<{ gdml: string; filename: string }>(
    '/api/document/export',
    { method: 'POST' },
  );
}
