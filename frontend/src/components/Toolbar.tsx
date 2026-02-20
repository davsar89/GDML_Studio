import { useAppStore } from '../store';
import * as api from '../api/client';
import { clearAllGeometries } from './Viewport/geometryCache';

export default function Toolbar() {
  const loading = useAppStore((s) => s.loading);
  const summary = useAppStore((s) => s.summary);
  const meshOpacity = useAppStore((s) => s.meshOpacity);
  const setMeshOpacity = useAppStore((s) => s.setMeshOpacity);

  const handleOpenFile = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.gdml';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;

      const store = useAppStore.getState();
      store.setLoading(true);
      store.setError(null);
      clearAllGeometries();

      try {
        const content = await file.text();
        const result = await api.uploadFile(file.name, content);
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
        store.setError(`Failed to load ${file.name}: ${msg}`);
      } finally {
        store.setLoading(false);
      }
    };
    input.click();
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
        <>
          <span style={{ fontSize: 12, color: '#8899aa' }}>
            {summary.filename} &mdash; {summary.solids_count} solids, {summary.volumes_count} volumes, {summary.meshes_count} meshes
          </span>
          <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 6 }}>
            <label style={{ fontSize: 11, color: '#8899aa' }}>Opacity</label>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={meshOpacity}
              onChange={(e) => setMeshOpacity(Number(e.target.value))}
              style={{ width: 100, accentColor: '#e94560' }}
            />
            <span style={{ fontSize: 11, color: '#b0b8c0', minWidth: 32, textAlign: 'right' }}>
              {Math.round(meshOpacity * 100)}%
            </span>
          </div>
        </>
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
