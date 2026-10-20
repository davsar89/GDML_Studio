import { useEffect } from 'react';
import { useAppStore } from '../store';
import * as api from '../api/client';
import { loadDocument } from '../utils/refresh';
import { runDocumentAction, resolveMaterialDraft, resolveDocumentReplacement, exportCurrentDocument, askFilename } from '../utils/documentActions';

export default function Toolbar() {
  const loading = useAppStore((s) => s.loading);
  const summary = useAppStore((s) => s.summary);
  const dirty = useAppStore((s) => s.dirty);
  const hasDraft = useAppStore((s) => s.materialDraft !== null);
  const documentBusy = useAppStore((s) => s.documentBusy);

  // A draft lives only in this tab and applied edits only in the backend's memory;
  // a reload restores the applied ones but loses the draft.
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
    // The desktop app's native Save dialog asks for the name itself.
    const current = useAppStore.getState().summary?.filename || 'output.gdml';
    const filename = window.desktop ? current : await askFilename(current);
    if (!filename || !await resolveMaterialDraft()) return;
    await exportCurrentDocument(filename);
  });

  const handleSave = () => runDocumentAction(async () => {
    if (await resolveMaterialDraft()) await exportCurrentDocument();
  });

  return (
    <header className="app-header">
      <div className="brand">
        <svg aria-hidden="true" width="28" height="28" viewBox="0 0 28 28" fill="none" stroke="currentColor" strokeWidth="1.5">
          <path d="M14 2 25 8.5v11L14 26 3 19.5v-11L14 2Zm0 13L3 8.5M14 15 25 8.5M14 15v11M8.5 5.25l11 6.5" />
        </svg>
        <h1>GDML Studio</h1>
      </div>
      <div className="document-name">
        <span className="truncate muted" title={summary?.filename}>{summary?.filename ?? 'Geometry workspace'}</span>
        {(dirty || hasDraft) && <span className="document-state" title="This document has edits that are not in any saved file">● {hasDraft ? 'unapplied changes' : 'unsaved'}</span>}
      </div>
      <div className="header-actions">
        <button className="primary" onClick={handleOpenFile} disabled={documentBusy} aria-busy={loading}>
          {loading ? 'Opening…' : 'Open File(s)'}
        </button>
        <button onClick={handleSave} disabled={!summary || documentBusy}>Save</button>
        <button className="quiet" onClick={handleSaveAs} disabled={!summary || documentBusy}>Save As</button>
      </div>
    </header>
  );
}

export function ViewportTools() {
  const loaded = useAppStore((s) => s.summary !== null);
  const meshOpacity = useAppStore((s) => s.meshOpacity);
  const measureMode = useAppStore((s) => s.measureMode);
  const measurements = useAppStore((s) => s.measurements);
  return (
    <div className="viewport-tools" aria-label="View controls">
      <button onClick={() => useAppStore.getState().setMeasureMode(!measureMode)} aria-pressed={measureMode} disabled={!loaded} title="Measure distance">
        <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          <path d="M2 22L22 2M6 18l2-2M10 14l2-2M14 10l2-2M18 6l2-2" />
        </svg>
        Measure
      </button>
      {measurements.length > 0 && <button className="quiet" onClick={() => useAppStore.getState().clearMeasurements()} title="Clear all measurements">Clear ({measurements.length})</button>}
      <label className="opacity-control">
        Opacity
        <input type="range" min={0} max={1} step={0.05} value={meshOpacity} disabled={!loaded}
          onChange={(e) => useAppStore.getState().setMeshOpacity(Number(e.target.value))} />
        <output>{Math.round(meshOpacity * 100)}%</output>
      </label>
    </div>
  );
}
