import { useRef } from 'react';
import { useAppStore } from '../../store';
import type { NistMaterial } from '../../store/types';
import * as api from '../../api/client';
import { importNistMaterial } from '../../utils/nistImport';
import SearchableCombobox from '../SearchableCombobox';

async function refreshMaterialsAndMeshes() {
  const matData = await api.getMaterials();
  const store = useAppStore.getState();
  store.setMaterials(matData.materials);
  store.setElements(matData.elements);
  const [meshData, structData] = await Promise.all([
    api.getMeshes(),
    api.getStructure(),
  ]);
  store.setMeshes(meshData.meshes);
  store.setSceneGraph(meshData.scene_graph);
  store.setVolumes(structData.volumes);
}

export default function VolumeDetail() {
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const volumes = useAppStore((s) => s.volumes);
  const materials = useAppStore((s) => s.materials);
  const comboboxRef = useRef<HTMLInputElement>(null);

  if (!selectedVolume) return null;

  const vol = volumes.find((v) => v.name === selectedVolume);
  if (!vol) return null;

  const mat = materials.find((m) => m.name === vol.material_ref);

  const handleMaterialSelect = async (name: string, isNist: boolean, nistData?: NistMaterial) => {
    try {
      if (isNist && nistData) {
        // Auto-import from NIST, then assign
        const newMat = await importNistMaterial(nistData);
        await api.addMaterial(newMat);
        await api.updateVolumeMaterialRef(vol.name, newMat.name);
      } else {
        await api.updateVolumeMaterialRef(vol.name, name);
      }
      await refreshMaterialsAndMeshes();
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
          <span style={{ color: '#8899aa', flexShrink: 0 }}>Material: </span>
          <SearchableCombobox
            value={vol.material_ref}
            materials={materials}
            onChange={handleMaterialSelect}
            inputRef={comboboxRef}
          />
        </div>
        {mat && (
          <div
            style={{
              marginTop: 4,
              padding: '3px 6px',
              background: '#1a1a2e',
              borderRadius: 3,
              color: '#8899aa',
              fontSize: 10,
              lineHeight: 1.5,
            }}
          >
            {mat.density != null && (
              <div>
                <span style={{ color: '#7a8a9a' }}>Density: </span>
                <span style={{ color: '#b0b8c0' }}>
                  {mat.density.value}{mat.density.unit ? ` ${mat.density.unit}` : ' g/cm3'}
                </span>
              </div>
            )}
            {mat.formula && (
              <div>
                <span style={{ color: '#7a8a9a' }}>Formula: </span>
                <span style={{ color: '#b0b8c0' }}>{mat.formula}</span>
              </div>
            )}
            {mat.z != null && (
              <div>
                <span style={{ color: '#7a8a9a' }}>Z: </span>
                <span style={{ color: '#b0b8c0' }}>{mat.z}</span>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
