import { useAppStore } from '../store';
import * as api from '../api/client';
import { clearAllGeometries } from './Viewport/geometryCache';

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

  const handleOpenFile = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.gdml';
    input.multiple = true;
    input.onchange = async () => {
      const fileList = input.files;
      if (!fileList || fileList.length === 0) return;

      const store = useAppStore.getState();
      store.setLoading(true);
      store.setError(null);
      clearAllGeometries();

      try {
        // Read all selected files
        const fileMap: Record<string, string> = {};
        for (let i = 0; i < fileList.length; i++) {
          const f = fileList[i];
          fileMap[f.name] = await f.text();
        }

        let result: Awaited<ReturnType<typeof api.uploadFile>>;

        if (fileList.length === 1) {
          // Single file — check if it references other files
          const name = fileList[0].name;
          const content = fileMap[name];
          const refs = findFileRefs(content);

          if (refs.length > 0) {
            // Auto-detect: prompt user (via warning) but still load what we can
            result = await api.uploadFile(name, content);
          } else {
            result = await api.uploadFile(name, content);
          }
        } else {
          // Multiple files — auto-detect main and use multi-upload
          const mainFile = detectMainFile(fileMap);
          result = await api.uploadFiles(fileMap, mainFile);
        }

        store.setSummary(result);

        const meshData = await api.getMeshes();
        store.setMeshes(meshData.meshes);
        store.setSceneGraph(meshData.scene_graph);

        const defData = await api.getDefines();
        store.setDefines(defData.defines);

        const structData = await api.getStructure();
        store.setVolumes(structData.volumes);
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        const names = Array.from(fileList).map((f) => f.name).join(', ');
        store.setError(`Failed to load ${names}: ${msg}`);
      } finally {
        store.setLoading(false);
      }
    };
    input.click();
  };

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
      <button onClick={handleOpenFile} disabled={loading} style={btnStyle}>
        {loading ? 'Loading...' : 'Open File(s)'}
      </button>
      {summary && (
        <>
          <span style={{ fontSize: 12, color: '#8899aa' }}>
            {summary.filename} &mdash; {summary.solids_count} solids, {summary.volumes_count} volumes, {summary.meshes_count} meshes
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
