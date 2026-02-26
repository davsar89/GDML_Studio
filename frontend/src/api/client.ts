import type { DocumentSummary, MeshData, SceneNode, DefineValue, VolumeInfo, MaterialInfo, ElementInfo, NistMaterial } from '../store/types';

const BASE = '';

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${url}`, init);
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `HTTP ${res.status}`);
  }
  return res.json();
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

export async function getSummary() {
  return fetchJson<DocumentSummary>('/api/document/summary');
}

export async function getMeshes() {
  return fetchJson<{
    meshes: Record<string, MeshData>;
    scene_graph: SceneNode;
  }>('/api/document/meshes');
}

export async function getDefines() {
  return fetchJson<{ defines: DefineValue[] }>('/api/document/defines');
}

export async function getMaterials() {
  return fetchJson<{ elements: ElementInfo[]; materials: MaterialInfo[] }>(
    '/api/document/materials',
  );
}

export async function getSolids() {
  return fetchJson<Record<string, unknown>>('/api/document/solids');
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

export async function updateElement(name: string, element: ElementInfo) {
  return fetchJson<{ ok: boolean }>('/api/document/elements/update', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, element }),
  });
}

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
