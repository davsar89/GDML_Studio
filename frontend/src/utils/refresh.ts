import * as api from '../api/client';
import { useAppStore } from '../store';
import { clearAllGeometries } from '../components/Viewport/geometryCache';

/** Refresh just the materials + elements lists from the backend. */
export function refreshMaterials(): Promise<void> {
  return api.getMaterials().then((data) => {
    const store = useAppStore.getState();
    store.setMaterials(data.materials);
    store.setElements(data.elements);
  });
}

/**
 * Refresh materials AND re-fetch meshes/structure after an edit that may have
 * changed geometry. Clears the geometry cache first: the cache is keyed by solid
 * name, so without clearing it would return stale geometry for changed solids
 * and never dispose orphaned ones.
 */
export async function refreshMaterialsAndMeshes(): Promise<void> {
  await refreshMaterials();
  try {
    const [meshData, structData] = await Promise.all([
      api.getMeshes(),
      api.getStructure(),
    ]);
    // Clear only after a successful re-fetch so a failure leaves the scene intact.
    clearAllGeometries();
    const store = useAppStore.getState();
    store.setMeshes(meshData.meshes);
    store.setSceneGraph(meshData.scene_graph);
    store.setVolumes(structData.volumes);
  } catch (e: unknown) {
    console.warn('Mesh/structure refresh failed:', e);
  }
}
