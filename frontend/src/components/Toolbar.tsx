import { useAppStore } from '../store';
import * as api from '../api/client';

export default function Toolbar() {
  const loading = useAppStore((s) => s.loading);
  const summary = useAppStore((s) => s.summary);

  const handleOpenFile = async () => {
    const path = prompt('Enter the full path to a GDML file:');
    if (!path) return;

    const store = useAppStore.getState();
    store.setLoading(true);
    store.setError(null);

    try {
      const result = await api.openFile(path);
      store.setSummary(result);

      const meshData = await api.getMeshes();
      store.setMeshes(meshData.meshes);
      store.setSceneGraph(meshData.scene_graph);

      const defData = await api.getDefines();
      store.setDefines(defData.defines);

      const structData = await api.getStructure();
      store.setVolumes(structData.volumes);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      store.setError(`Failed to load ${path}: ${msg}`);
    } finally {
      store.setLoading(false);
    }
  };

  return (
    <div
      style={{
        height: 40,
        background: '#16213e',
        borderBottom: '1px solid #0f3460',
        display: 'flex',
        alignItems: 'center',
        padding: '0 12px',
        gap: 12,
        flexShrink: 0,
      }}
    >
      <span style={{ fontWeight: 700, fontSize: 14, color: '#e94560', marginRight: 8 }}>
        GDML Studio
      </span>
      <button onClick={handleOpenFile} disabled={loading} style={btnStyle}>
        {loading ? 'Loading...' : 'Open File'}
      </button>
      {summary && (
        <span style={{ fontSize: 12, color: '#8899aa' }}>
          {summary.filename} &mdash; {summary.solids_count} solids, {summary.volumes_count} volumes, {summary.meshes_count} meshes
        </span>
      )}
    </div>
  );
}

const btnStyle: React.CSSProperties = {
  background: '#0f3460',
  color: '#e0e0e0',
  border: '1px solid #1a1a4e',
  borderRadius: 4,
  padding: '4px 12px',
  cursor: 'pointer',
  fontSize: 13,
};
