import * as THREE from 'three';
import type { MeshData } from '../../store/types';

const cache = new Map<string, { geometry: THREE.BufferGeometry; refCount: number }>();

export function getOrCreateGeometry(solidName: string, meshData: MeshData): THREE.BufferGeometry {
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

export function releaseGeometry(solidName: string): void {
  const entry = cache.get(solidName);
  if (!entry) return;

  entry.refCount--;
  if (entry.refCount <= 0) {
    entry.geometry.dispose();
    cache.delete(solidName);
  }
}

export function clearAllGeometries(): void {
  for (const entry of cache.values()) {
    entry.geometry.dispose();
  }
  cache.clear();
}
