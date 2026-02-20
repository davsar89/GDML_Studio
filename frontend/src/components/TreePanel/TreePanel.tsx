import { useState } from 'react';
import VolumeTree from './VolumeTree';
import DefinesTree from './DefinesTree';

type Tab = 'structure' | 'defines';

export default function TreePanel() {
  const [tab, setTab] = useState<Tab>('structure');

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
        {(['structure', 'defines'] as Tab[]).map((t) => (
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
        {tab === 'structure' && <VolumeTree />}
        {tab === 'defines' && <DefinesTree />}
      </div>
    </div>
  );
}
