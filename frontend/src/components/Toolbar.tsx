import { useEffect } from 'react';
import { useAppStore } from '../store';
import * as api from '../api/client';
import { clearAllGeometries } from './Viewport/geometryCache';
import { runDocumentAction, resolveMaterialDraft, resolveDocumentReplacement, exportCurrentDocument } from '../utils/documentActions';

/** Scan GDML text for <file name="..."> references and return the referenced filenames. */
function findFileRefs(content: string): string[] {
  const refs: string[] = [];
  const re = /<file\s[^>]*name\s*=\s*"([^"]+)"/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(content)) !== null) {
    refs.push(m[1]);
  }
  return refs;
}

/**
 * Given a map of filename→content, determine the "main" file.
 * The main file is the one that references other files via <file> but is not
 * itself referenced by any other file.  Falls back to the first file.
 */
function detectMainFile(files: Record<string, string>): string {
  const names = Object.keys(files);
  if (names.length === 1) return names[0];

  const referencedNames = new Set<string>();
  const refCountMap: Record<string, number> = {};

  for (const [name, content] of Object.entries(files)) {
    const refs = findFileRefs(content);
    refCountMap[name] = refs.length;
    for (const r of refs) referencedNames.add(r);
  }

  // Main = references others but is not referenced itself
  for (const name of names) {
    if (refCountMap[name] > 0 && !referencedNames.has(name)) {
      return name;
    }
  }
  // Fallback: the one with the most references
  return names.sort((a, b) => (refCountMap[b] || 0) - (refCountMap[a] || 0))[0];
}

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
  const unavailable = loading || documentBusy;

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
      const fileList = input.files;
      if (!fileList || fileList.length === 0) return;

      await runDocumentAction(async () => {
        if (!await resolveDocumentReplacement()) return;
        const store = useAppStore.getState();
        store.setLoading(true);
        let uploaded = false;
        try {
          const fileMap: Record<string, string> = {};
          for (const file of Array.from(fileList)) fileMap[file.name] = await file.text();
          const result = fileList.length === 1
            ? await api.uploadFile(fileList[0].name, fileMap[fileList[0].name])
            : await api.uploadFiles(fileMap, detectMainFile(fileMap));
          uploaded = true;
          const [meshData, defData, structData, matData] = await Promise.all([
            api.getMeshes(), api.getDefines(), api.getStructure(), api.getMaterials(),
          ]);
          clearAllGeometries();
          useAppStore.setState({
            summary: result, warnings: [...(result.warnings ?? []), ...(meshData.warnings ?? [])],
            meshes: meshData.meshes, sceneGraph: meshData.scene_graph,
            defines: defData.defines, volumes: structData.volumes,
            materials: matData.materials, elements: matData.elements,
            selectedVolume: null, selectedMaterial: null, hiddenInstances: new Set(),
            materialDraft: null, draftError: null, dirty: false, editRevision: 0,
            measureMode: false, measurements: [], pendingPoint: null,
            hoverSnap: null, hoverCandidates: [], surfaceCandidates: [], contextMenu: null,
          });
        } catch (e: unknown) {
          // A rejected upload leaves the previous backend document intact.
          // If fetching its replacement failed, discard the mismatched preview.
          if (uploaded) store.reset();
          const message = e instanceof Error ? e.message : String(e);
          store.setError(`Failed to load ${Array.from(fileList).map((f) => f.name).join(', ')}: ${message}`);
        } finally {
          store.setLoading(false);
        }
      });
    };
    input.click();
  };

  const handleSaveAs = () => runDocumentAction(async () => {
    // Cancelling the filename prompt leaves even an unapplied draft intact.
    const filename = prompt('Save as:', useAppStore.getState().summary?.filename || 'output.gdml');
    if (!filename || !await resolveMaterialDraft()) return;
    await exportCurrentDocument(filename);
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
      <button onClick={handleOpenFile} disabled={unavailable} style={btnStyle}>
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
          <button onClick={handleSave} disabled={unavailable} style={btnStyle}>Save</button>
          <button onClick={handleSaveAs} disabled={unavailable} style={btnStyle}>Save As</button>
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
            <label style={{ fontSize: 11, color: '#8899aa' }}>Opacity</label>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={meshOpacity}
              onChange={(e) => setMeshOpacity(Number(e.target.value))}
              style={{ width: 100, accentColor: '#e94560' }}
            />
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
