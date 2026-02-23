import { useAppStore } from '../../store';
import * as api from '../../api/client';

export default function VolumeDetail() {
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const volumes = useAppStore((s) => s.volumes);
  const materials = useAppStore((s) => s.materials);

  if (!selectedVolume) return null;

  const vol = volumes.find((v) => v.name === selectedVolume);
  if (!vol) return null;

  const handleMaterialChange = async (newRef: string) => {
    try {
      await api.updateVolumeMaterialRef(vol.name, newRef);
      // Re-fetch structure and meshes for updated coloring
      const structData = await api.getStructure();
      const store = useAppStore.getState();
      store.setVolumes(structData.volumes);
      const meshData = await api.getMeshes();
      store.setMeshes(meshData.meshes);
      store.setSceneGraph(meshData.scene_graph);
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div
      style={{
        borderTop: '1px solid #0f3460',
        marginTop: 8,
        paddingTop: 6,
      }}
    >
      <div style={{ fontSize: 11, fontWeight: 700, color: '#8899aa', marginBottom: 4 }}>
        Volume Detail
      </div>
      <div style={{ fontSize: 10, fontFamily: 'monospace', color: '#b0b8c0' }}>
        <div style={{ marginBottom: 2 }}>
          <span style={{ color: '#8899aa' }}>Name: </span>
          <span style={{ color: '#56d6c8' }}>{vol.name}</span>
        </div>
        <div style={{ marginBottom: 2 }}>
          <span style={{ color: '#8899aa' }}>Solid: </span>
          <span>{vol.solid_ref}</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <span style={{ color: '#8899aa' }}>Material: </span>
          <select
            value={vol.material_ref}
            onChange={(e) => handleMaterialChange(e.target.value)}
            style={{
              background: '#1a1a2e',
              color: '#e0e0e0',
              border: '1px solid #0f3460',
              borderRadius: 3,
              padding: '1px 4px',
              fontSize: 10,
              fontFamily: 'monospace',
              outline: 'none',
            }}
          >
            {materials.map((m) => (
              <option key={m.name} value={m.name}>{m.name}</option>
            ))}
            {/* If current material_ref doesn't match any loaded material, show it anyway */}
            {!materials.some((m) => m.name === vol.material_ref) && (
              <option value={vol.material_ref}>{vol.material_ref}</option>
            )}
          </select>
        </div>
      </div>
    </div>
  );
}
