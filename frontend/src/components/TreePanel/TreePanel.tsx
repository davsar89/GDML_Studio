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
      {/*
        `minHeight: 0` lets this flex child actually shrink, which is what gives
        the scrollers below a bounded height. DefinesTree owns its own scroller
        because it virtualises against it; the other two tabs get one here.
      */}
      <div style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
        {tab === 'structure' && (
          <div style={{ flex: 1, minHeight: 0, overflow: 'auto', padding: 8 }}>
            <VolumeTree />
            <VolumeDetail />
          </div>
        )}
        {tab === 'defines' && <DefinesTree />}
        {tab === 'materials' && (
          <div style={{ flex: 1, minHeight: 0, overflow: 'auto', padding: 8 }}>
            <MaterialsPanel />
          </div>
        )}
      </div>
    </div>
  );
}
