import { useAppStore } from '../../store';
import VolumeTree from './VolumeTree';
import DefinesTree from './DefinesTree';
import MaterialsPanel from './MaterialsPanel';
import VolumeDetail from './VolumeDetail';

type Tab = 'structure' | 'defines' | 'materials';

export default function TreePanel() {
  const tab = useAppStore((s) => s.activeTreeTab);
  const setTab = useAppStore((s) => s.setActiveTreeTab);

  return (
    <div
      style={{
        background: '#16213e',
        borderRight: '1px solid #0f3460',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <div style={{ display: 'flex', borderBottom: '1px solid #0f3460' }}>
        {(['structure', 'defines', 'materials'] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            style={{
              flex: 1,
              padding: '6px 0',
              background: tab === t ? '#1a1a2e' : 'transparent',
              color: tab === t ? '#e94560' : '#8899aa',
              border: 'none',
              cursor: 'pointer',
              fontSize: 12,
              fontWeight: tab === t ? 700 : 400,
              textTransform: 'capitalize',
            }}
          >
            {t}
          </button>
        ))}
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: 8 }}>
        {tab === 'structure' && (
          <>
            <VolumeTree />
            <VolumeDetail />
          </>
        )}
        {tab === 'defines' && <DefinesTree />}
        {tab === 'materials' && <MaterialsPanel />}
      </div>
    </div>
  );
}
