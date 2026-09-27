import { useRef } from 'react';
import { useAppStore } from '../../store';
import type { NistMaterial } from '../../store/types';
import * as api from '../../api/client';
import { prepareNistMaterial } from '../../utils/nistImport';
import { refreshMaterialsAndScene } from '../../utils/refresh';
import { runDocumentAction, resolveMaterialDraft } from '../../utils/documentActions';
import SearchableCombobox from '../SearchableCombobox';

export default function VolumeDetail() {
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const volumes = useAppStore((s) => s.volumes);
  const materials = useAppStore((s) => s.materials);
  const busy = useAppStore((s) => s.documentBusy || s.loading);
  const comboboxRef = useRef<HTMLInputElement>(null);

  if (!selectedVolume) return null;

  const vol = volumes.find((v) => v.name === selectedVolume);
  if (!vol) return null;

  const mat = materials.find((m) => m.name === vol.material_ref);

  const handleMaterialSelect = async (name: string, isNist: boolean, nistData?: NistMaterial) => {
    if (name === vol.material_ref) return;
    await runDocumentAction(async () => {
      if (!await resolveMaterialDraft()) return;
      try {
        if (isNist && nistData) {
          // Auto-import from NIST under its own name, then assign
          const { material, elements } = await prepareNistMaterial(nistData);
          await api.addMaterial(material, elements);
        }
        await api.updateVolumeMaterialRef(vol.name, name);
      } finally {
        // Even a half-done NIST pick may have added the material.
        await refreshMaterialsAndScene();
      }
    });
  };

  return (
    <fieldset disabled={busy}
      style={{
        margin: 0, padding: 0, border: 0, minWidth: 0,
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
    </fieldset>
  );
}
