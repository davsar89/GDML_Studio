import { useAppStore } from '../store';
import VolumeDetail from './TreePanel/VolumeDetail';
import { MaterialEditor } from './TreePanel/MaterialsPanel';

export default function Inspector() {
  const tab = useAppStore((s) => s.activeTreeTab);
  const selectedMaterial = useAppStore((s) => s.selectedMaterial);
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const materials = useAppStore((s) => s.materials);
  const elements = useAppStore((s) => s.elements);
  const busy = useAppStore((s) => s.documentBusy);
  const editingMaterial = tab === 'materials';
  const selected = editingMaterial ? selectedMaterial : selectedVolume;

  return (
    <aside className="inspector" aria-label="Inspector">
      <div className="panel-heading"><h2>Inspector</h2><span className="muted">{editingMaterial ? 'Material' : 'Volume'}</span></div>
      <div className="panel-scroll">
        {!selected ? (
          <div className="empty-state"><strong>{editingMaterial ? 'Select a material' : 'Select a volume'}</strong>
            {editingMaterial ? 'Choose a material in the explorer to view and edit its properties.' : 'Choose a volume in the explorer or the 3D view to inspect it.'}
          </div>
        ) : editingMaterial ? (
          <fieldset disabled={busy}>
            <MaterialEditor key={selectedMaterial} materialName={selectedMaterial!} materials={materials} elements={elements} />
          </fieldset>
        ) : <VolumeDetail />}
      </div>
    </aside>
  );
}
