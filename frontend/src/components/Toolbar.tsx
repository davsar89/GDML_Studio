import { useEffect } from 'react';
import { useAppStore } from '../store';
import * as api from '../api/client';
import { loadDocument } from '../utils/refresh';
import { runDocumentAction, resolveMaterialDraft, resolveDocumentReplacement, exportCurrentDocument, askFilename } from '../utils/documentActions';

export default function Toolbar() {
  const loading = useAppStore((s) => s.loading);
  const summary = useAppStore((s) => s.summary);
  const meshOpacity = useAppStore((s) => s.meshOpacity);
  const setMeshOpacity = useAppStore((s) => s.setMeshOpacity);
  const measureMode = useAppStore((s) => s.measureMode);
  const measurements = useAppStore((s) => s.measurements);
  const dirty = useAppStore((s) => s.dirty);
  const hasDraft = useAppStore((s) => s.materialDraft !== null);
  const documentBusy = useAppStore((s) => s.documentBusy);

  // Edits live only in the backend's in-memory document; closing the tab
  // discards them, and nothing else warns about that.
  useEffect(() => {
    if (!dirty && !hasDraft) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener('beforeunload', onBeforeUnload);
    return () => window.removeEventListener('beforeunload', onBeforeUnload);
  }, [dirty, hasDraft]);

  const handleOpenFile = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.gdml';
    input.multiple = true;
    input.onchange = async () => {
      const files = Array.from(input.files ?? []);
      if (files.length === 0) return;
      await runDocumentAction(async () => {
        if (!await resolveDocumentReplacement()) return;
        await loadDocument(async () => api.uploadFiles(Object.fromEntries(
          await Promise.all(files.map(async (file) => [file.name, await file.text()])),
        )), files.map((file) => file.name).join(', '));
      });
    };
    input.click();
  };

  const handleSaveAs = () => runDocumentAction(async () => {
    // Cancelling the file name leaves even an unapplied draft intact.
    const filename = await askFilename(useAppStore.getState().summary?.filename || 'output.gdml');
    if (!filename || !await resolveMaterialDraft()) return;
    await exportCurrentDocument(filename);
    // The next Save keeps the name chosen here.
    useAppStore.setState((s) => ({ summary: s.summary && { ...s.summary, filename } }));
  });

  const handleSave = () => runDocumentAction(async () => {
    if (await resolveMaterialDraft()) await exportCurrentDocument();
  });

  return (
    <div
      style={{
        height: 40,
        background: '#16213e',
        borderBottom: '1px solid #0f3460',
        display: 'flex',
        alignItems: 'center',
        padding: '0 12px',
        gap: 12,
        flexShrink: 0,
      }}
    >
      <span style={{ fontWeight: 700, fontSize: 14, color: '#e94560', marginRight: 8 }}>
        GDML Studio
      </span>
      <button onClick={handleOpenFile} disabled={documentBusy} style={btnStyle}>
        {loading ? 'Loading...' : 'Open File(s)'}
      </button>
      <button
        onClick={() => useAppStore.getState().setMeasureMode(!measureMode)}
        style={{
          ...btnStyle,
          background: measureMode ? '#e94560' : '#0f3460',
          color: measureMode ? '#fff' : '#e0e0e0',
        }}
        title="Measure distance"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ verticalAlign: 'middle', marginRight: 4 }}>
          <path d="M2 22L22 2" />
          <path d="M6 18l2-2" />
          <path d="M10 14l2-2" />
          <path d="M14 10l2-2" />
          <path d="M18 6l2-2" />
        </svg>
        Measure
      </button>
      {measurements.length > 0 && (
        <button onClick={() => useAppStore.getState().clearMeasurements()} style={btnStyle} title="Clear all measurements">
          Clear ({measurements.length})
        </button>
      )}
      {summary && (
        <>
          <button onClick={handleSave} disabled={documentBusy} style={btnStyle}>Save</button>
          <button onClick={handleSaveAs} disabled={documentBusy} style={btnStyle}>Save As</button>
          <span style={{ fontSize: 12, color: '#8899aa' }}>
            {summary.filename}
            {(dirty || hasDraft) && (
              <span
                style={{ color: '#e9a545', marginLeft: 4 }}
                title="This document has edits that are not in any saved file"
              >
                &bull; {hasDraft ? 'unapplied changes' : 'unsaved'}
              </span>
            )}
            {' — '}
            {summary.solids_count} solids, {summary.volumes_count} volumes, {summary.meshes_count} meshes
          </span>
          <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 6 }}>
            <label style={{ fontSize: 11, color: '#8899aa', display: 'flex', alignItems: 'center', gap: 6 }}>
              Opacity
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={meshOpacity}
                onChange={(e) => setMeshOpacity(Number(e.target.value))}
                style={{ width: 100, accentColor: '#e94560' }}
              />
            </label>
            <span style={{ fontSize: 11, color: '#b0b8c0', minWidth: 32, textAlign: 'right' }}>
              {Math.round(meshOpacity * 100)}%
            </span>
          </div>
        </>
      )}
    </div>
  );
}

const btnStyle: React.CSSProperties = {
  background: '#0f3460',
  color: '#e0e0e0',
  border: '1px solid #1a1a4e',
  borderRadius: 4,
  padding: '4px 12px',
  cursor: 'pointer',
  fontSize: 13,
};
