import * as api from '../api/client';
import { useAppStore } from '../store';

/** Refresh just the materials + elements lists from the backend. */
export function refreshMaterials(): Promise<void> {
  return api.getMaterials().then((data) => {
    const store = useAppStore.getState();
    store.setMaterials(data.materials);
    store.setElements(data.elements);
  });
}

/** Refresh material data, placement references and colours; retain unchanged mesh buffers. */
export async function refreshMaterialsAndScene(): Promise<void> {
  await refreshMaterials();
  try {
    const [sceneData, structData] = await Promise.all([
      api.getScene(),
      api.getStructure(),
    ]);
    const store = useAppStore.getState();
    store.setSceneGraph(sceneData.scene_graph);
    store.setVolumes(structData.volumes);
  } catch (e: unknown) {
    // Surfaced rather than logged: a silent failure here leaves the materials
    // list updated while the 3D view still shows pre-edit colours.
    useAppStore
      .getState()
      .setError(
        `Could not refresh the view after the edit: ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
  }
}
