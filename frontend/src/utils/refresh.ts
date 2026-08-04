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

/**
 * Refresh materials and the derived scene graph after a material/element edit.
 *
 * Deliberately does NOT refetch meshes. `update_material` only mutates the
 * materials list — tessellation runs solely on upload — so the vertex data is
 * unchanged. Refetching it meant every debounced keystroke pulled the entire
 * mesh payload (~16 MB on the largest sample) and, because `setMeshes` installs
 * fresh arrays and MeshObject keys its geometry effect on `[solidName,
 * meshData]`, rebuilt and re-uploaded every BufferGeometry in the document.
 *
 * The scene graph still has to come back: node colour is derived from density.
 */
export async function refreshMaterialsAndMeshes(): Promise<void> {
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
