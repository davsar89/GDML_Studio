import { useAppStore } from '../../store';
import VolumeTree from './VolumeTree';
import DefinesTree from './DefinesTree';
import MaterialsPanel from './MaterialsPanel';

export default function TreePanel() {
  const tab = useAppStore((s) => s.activeTreeTab);
  const setTab = useAppStore((s) => s.setActiveTreeTab);
  return (
    <aside className="navigation" aria-label="Document navigation">
      <div className="panel-heading"><h2>Explorer</h2><span className="muted">GDML</span></div>
      <nav className="tabs" aria-label="Document sections">
        {(['structure', 'defines', 'materials'] as const).map((t) => (
          <button key={t} onClick={() => setTab(t)} aria-pressed={tab === t}>
            {t[0].toUpperCase() + t.slice(1)}
          </button>
        ))}
      </nav>
      <div className="panel-body">
        {tab === 'structure' && <div className="panel-scroll"><VolumeTree /></div>}
        {tab === 'defines' && <DefinesTree />}
        {tab === 'materials' && <div className="panel-scroll"><MaterialsPanel /></div>}
      </div>
    </aside>
  );
}
