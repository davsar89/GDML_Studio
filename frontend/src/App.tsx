import { useAppStore } from './store';
import Toolbar from './components/Toolbar';
import Layout from './components/Layout';
import ContextMenu from './components/ContextMenu';

export default function App() {
  const error = useAppStore((s) => s.error);
  const warnings = useAppStore((s) => s.warnings);
  const clearWarnings = useAppStore((s) => s.clearWarnings);

  return (
    <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', fontFamily: 'system-ui, sans-serif', color: '#e0e0e0', background: '#1a1a2e' }}>
      <Toolbar />
      {error && (
        <div style={{ background: '#b71c1c', color: '#fff', padding: '8px 16px', fontSize: '13px' }}>
          Error: {error}
          <button onClick={() => useAppStore.getState().setError(null)} style={{ marginLeft: 16, background: 'none', border: '1px solid #fff', color: '#fff', cursor: 'pointer', borderRadius: 3, padding: '2px 8px' }}>
            Dismiss
          </button>
        </div>
      )}
      {warnings.length > 0 && (
        <div style={{ background: '#6b4f1d', color: '#fff3d6', padding: '8px 16px', fontSize: '13px', display: 'flex', gap: 16, alignItems: 'flex-start' }}>
          <div style={{ flex: 1 }}>
            <strong style={{ display: 'block', marginBottom: 4 }}>Warnings</strong>
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {warnings.map((warning, idx) => (
                <li key={`${idx}-${warning}`}>{warning}</li>
              ))}
            </ul>
          </div>
          <button
            onClick={clearWarnings}
            style={{ background: 'none', border: '1px solid #fff3d6', color: '#fff3d6', cursor: 'pointer', borderRadius: 3, padding: '2px 8px' }}
          >
            Dismiss
          </button>
        </div>
      )}
      <Layout />
      <ContextMenu />
    </div>
  );
}
