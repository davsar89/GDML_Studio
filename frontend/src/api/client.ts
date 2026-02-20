import type { DocumentSummary, MeshData, SceneNode, DefineValue, VolumeInfo } from '../store/types';

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
  return fetchJson<Record<string, unknown>>('/api/document/materials');
}

export async function getSolids() {
  return fetchJson<Record<string, unknown>>('/api/document/solids');
}

export async function getStructure() {
  return fetchJson<{ volumes: VolumeInfo[]; world_ref: string }>(
    '/api/document/structure',
  );
}
