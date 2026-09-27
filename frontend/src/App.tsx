import { useEffect } from 'react';
import { useAppStore } from './store';
import { runDocumentAction } from './utils/documentActions';
import { restoreOpenDocument } from './utils/refresh';
import Toolbar from './components/Toolbar';
import Layout from './components/Layout';
import ContextMenu from './components/ContextMenu';
import DocumentDialog from './components/DocumentDialog';

export default function App() {
  const error = useAppStore((s) => s.error);
  const warnings = useAppStore((s) => s.warnings);
  const clearWarnings = useAppStore((s) => s.clearWarnings);

  // The backend outlives a page reload; runDocumentAction ignores StrictMode's second call.
  useEffect(() => { void runDocumentAction(restoreOpenDocument); }, []);

  return (
    <div className="app">
      <Toolbar />
      {error && (
        <div className="notice error-notice" role="alert">
          <span><strong>Error:</strong> {error}</span>
          <button className="quiet" onClick={() => useAppStore.getState().setError(null)}>
            Dismiss
          </button>
        </div>
      )}
      {warnings.length > 0 && (
        <div className="notice warning-notice">
          <details>
            <summary><strong>Geometry warnings ({warnings.length})</strong><span className="warning-preview">{warnings[0]}</span></summary>
            <ul>
              {warnings.map((warning, idx) => (
                <li key={`${idx}-${warning}`}>{warning}</li>
              ))}
            </ul>
          </details>
          <button
            onClick={clearWarnings}
            className="quiet"
          >
            Dismiss
          </button>
        </div>
      )}
      <Layout />
      <ContextMenu />
      <DocumentDialog />
    </div>
  );
}
