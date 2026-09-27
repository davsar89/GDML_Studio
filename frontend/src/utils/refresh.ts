import * as api from '../api/client';
import { useAppStore, emptyDocument } from '../store';
import type { DocumentSummary } from '../store/types';
import { clearAllGeometries } from '../components/Viewport/geometryCache';

/**
 * Show the document `upload` makes current in the backend, replacing the
 * client's copy wholesale. `what` names it in the error message.
 */
export async function loadDocument(upload: () => Promise<DocumentSummary>, what: string): Promise<void> {
  const store = useAppStore.getState();
  useAppStore.setState({ loading: true });
  let uploaded = false;
  try {
    const summary = await upload();
    uploaded = true;
    const [meshData, defData, structData, matData] = await Promise.all([
      api.getMeshes(), api.getDefines(), api.getStructure(), api.getMaterials(),
    ]);
    clearAllGeometries();
    useAppStore.setState({
      ...emptyDocument(),
      summary, warnings: [...(summary.warnings ?? []), ...(meshData.warnings ?? [])],
      meshes: meshData.meshes, sceneGraph: meshData.scene_graph,
      defines: defData.defines, volumes: structData.volumes,
      materials: matData.materials, elements: matData.elements,
    });
  } catch (e: unknown) {
    // A rejected upload leaves the previous backend document intact.
    // If fetching its replacement failed, discard the mismatched preview.
    if (uploaded) store.reset();
    store.setError(`Failed to load ${what}: ${e instanceof Error ? e.message : String(e)}`);
  } finally {
    useAppStore.setState({ loading: false });
  }
}

/** After a page reload, show the document the backend still holds. */
export async function restoreOpenDocument(): Promise<void> {
  // Nothing loaded (404) or no backend: start empty, as before any Open.
  const summary = await api.getSummary().catch(() => null);
  if (summary) await loadDocument(async () => summary, summary.filename);
}

/** Refresh just the materials + elements lists from the backend. */
export function refreshMaterials(): Promise<void> {
  return api.getMaterials().then((data) => useAppStore.setState({ materials: data.materials, elements: data.elements }));
}

/** Refresh material data, placement references and colours; retain unchanged mesh buffers. */
export async function refreshMaterialsAndScene(): Promise<void> {
  await refreshMaterials();
  try {
    const [sceneData, structData] = await Promise.all([api.getScene(), api.getStructure()]);
    useAppStore.setState({ sceneGraph: sceneData.scene_graph, volumes: structData.volumes });
  } catch (e: unknown) {
    // Surfaced rather than logged: a silent failure here leaves the materials
    // list updated while the 3D view still shows pre-edit colours.
    const message = e instanceof Error ? e.message : String(e);
    useAppStore.getState().setError(`Could not refresh the view after the edit: ${message}`);
  }
}
