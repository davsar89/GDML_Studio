import * as THREE from 'three';
import type { MeshData } from '../../store/types';

const cache = new Map<string, { geometry: THREE.BufferGeometry; refCount: number }>();

/**
 * Acquire the geometry for a solid, incrementing its reference count.
 * MUST be balanced by exactly one `releaseGeometry(solidName)` call.
 * Call this once per consumer lifetime (e.g. in a mount effect), NOT in a
 * React render body, or the ref count grows on every re-render and the
 * geometry is never disposed.
 */
export function acquireGeometry(solidName: string, meshData: MeshData): THREE.BufferGeometry {
  const entry = cache.get(solidName);
  if (entry) {
    entry.refCount++;
    return entry.geometry;
  }

  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(meshData.positions, 3));
  geo.setAttribute('normal', new THREE.Float32BufferAttribute(meshData.normals, 3));
  geo.setIndex(new THREE.Uint32BufferAttribute(meshData.indices, 1));

  cache.set(solidName, { geometry: geo, refCount: 1 });
  return geo;
}

/**
 * Read the cached geometry for a solid WITHOUT changing its reference count.
 * Returns undefined if it has not been acquired yet. Safe to call during render.
 */
export function peekGeometry(solidName: string): THREE.BufferGeometry | undefined {
  return cache.get(solidName)?.geometry;
}

export function releaseGeometry(solidName: string): void {
  const entry = cache.get(solidName);
  if (!entry) return;

  entry.refCount--;
  if (entry.refCount <= 0) {
    entry.geometry.dispose();
    cache.delete(solidName);
  }
}

const materials = new Map<string, THREE.MeshStandardMaterial>();

/**
 * One material per look, shared by every mesh with that look. A deeper volume is offset
 * further toward the camera, so a daughter wins the faces it shares with its mother.
 * Ceiling: one entry per opacity step used, freed only on load or reset; key on a quantised opacity if that grows.
 */
export function sharedMaterial(color: string, selected: boolean, opacity: number, depth: number): THREE.MeshStandardMaterial {
  const key = `${color}|${selected}|${opacity}|${depth}`;
  if (!materials.has(key)) materials.set(key, new THREE.MeshStandardMaterial({
    color: selected ? '#8eb4ff' : color, emissive: selected ? '#3268b8' : color, emissiveIntensity: selected ? 0.3 : 0.12,
    metalness: 0.15, roughness: 0.45, side: THREE.DoubleSide, transparent: opacity < 1, opacity, depthWrite: opacity >= 1,
    polygonOffset: true, polygonOffsetFactor: -depth, polygonOffsetUnits: -depth,
  }));
  return materials.get(key)!;
}

/** Frees every cached geometry and material; call when the document is replaced. */
export function clearAllGeometries(): void {
  for (const entry of cache.values()) {
    entry.geometry.dispose();
  }
  cache.clear();
  for (const material of materials.values()) material.dispose();
  materials.clear();
}
