import { useAppStore } from './store';
import Toolbar from './components/Toolbar';
import Layout from './components/Layout';

export default function App() {
  const error = useAppStore((s) => s.error);

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
      <Layout />
    </div>
  );
}
