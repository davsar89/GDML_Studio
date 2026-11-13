import { useAppStore } from '../../store';
import type { TreeTab } from '../../store';
import VolumeTree from './VolumeTree';
import DefinesTree from './DefinesTree';
import MaterialsPanel from './MaterialsPanel';
import ImportPanel from './ImportPanel';

export default function TreePanel() {
  const tab = useAppStore((s) => s.activeTreeTab);
  const setTab = useAppStore((s) => s.setActiveTreeTab);
  const imported = useAppStore((s) => s.importReport !== null);
  const tabs: TreeTab[] = ['structure', 'defines', 'materials', ...(imported ? ['import' as const] : [])];
  // A document opened after an import has no Import tab.
  const shown = tabs.includes(tab) ? tab : 'structure';
  return (
    <aside className="navigation" aria-label="Document navigation">
      <div className="panel-heading"><h2>Explorer</h2><span className="muted">GDML</span></div>
      <nav className="tabs" aria-label="Document sections">
        {tabs.map((t) => (
          <button key={t} onClick={() => setTab(t)} aria-pressed={shown === t}>
            {t[0].toUpperCase() + t.slice(1)}
          </button>
        ))}
      </nav>
      <div className="panel-body">
        {shown === 'structure' && <div className="panel-scroll"><VolumeTree /></div>}
        {shown === 'defines' && <DefinesTree />}
        {shown === 'materials' && <div className="panel-scroll"><MaterialsPanel /></div>}
        {shown === 'import' && <div className="panel-scroll"><ImportPanel /></div>}
      </div>
    </aside>
  );
}
