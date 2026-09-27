import { create } from 'zustand';
import { runDocumentAction, resolveMaterialDraft } from '../utils/documentActions';
import type { DocumentDecision } from '../utils/documentActions';
import type { SceneNode, MeshData, DocumentSummary, DefineValue, VolumeInfo, MaterialInfo, ElementInfo, SnapPoint, Measurement } from './types';
import { clearAllGeometries } from '../components/Viewport/geometryCache';

interface AppState {
  /** Set only by loadDocument, which runs inside a document action, so `documentBusy` covers it. */
  loading: boolean;
  error: string | null;
  warnings: string[];
  /**
   * The in-memory document has edits that are not in any file on disk.
   *
   * Set by every mutating API call (see `markDirty` in `api/client.ts`) and
   * cleared on load and on a successful export. Note "Save" downloads a copy
   * rather than overwriting the file that was opened, so clearing here means
   * "the user has been given a file matching this state", not "the original
   * path is up to date".
   */
  dirty: boolean;
  editRevision: number;
  documentBusy: boolean;
  decision: DocumentDecision | null;
  materialDraft: { original: MaterialInfo; material: MaterialInfo; elements: ElementInfo[] } | null;
  draftError: string | null;
  updateMaterialDraft: (changes: Partial<MaterialInfo>, elements?: ElementInfo[]) => void;
  discardMaterialDraft: () => void;
  summary: DocumentSummary | null;
  meshes: Record<string, MeshData>;
  sceneGraph: SceneNode | null;
  defines: DefineValue[];
  volumes: VolumeInfo[];
  materials: MaterialInfo[];
  elements: ElementInfo[];
  selectedVolume: string | null;
  selectedMaterial: string | null;
  activeTreeTab: 'structure' | 'defines' | 'materials';
  meshOpacity: number;
  hiddenInstances: Set<string>;
  contextMenu: {
    x: number;
    y: number;
    items: { label: string; action: () => void }[];
  } | null;

  // Measurement state
  measureMode: boolean;
  measurements: Measurement[];
  pendingPoint: SnapPoint | null;
  hoverSnap: SnapPoint | null;
  hoverCandidates: SnapPoint[];
  surfaceCandidates: SnapPoint[];

  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  markDirty: () => void;
  markSaved: (revision?: number) => void;
  clearWarnings: () => void;
  setSelectedVolume: (name: string | null) => void;
  setSelectedMaterial: (name: string | null) => void;
  setActiveTreeTab: (tab: 'structure' | 'defines' | 'materials') => void;
  setMeshOpacity: (opacity: number) => void;
  toggleInstanceVisibility: (instanceId: string) => void;
  openContextMenu: (x: number, y: number, items: { label: string; action: () => void }[]) => void;
  closeContextMenu: () => void;

  // Measurement actions
  setMeasureMode: (on: boolean) => void;
  placeMeasurePoint: (snap: SnapPoint) => void;
  cancelMeasure: () => void;
  clearMeasurements: () => void;

  reset: () => void;
}

/** Everything that belongs to the open document; loading or resetting replaces all of it. */
export const emptyDocument = () => ({
  summary: null, warnings: [], meshes: {}, sceneGraph: null, defines: [], volumes: [], materials: [], elements: [],
  dirty: false, editRevision: 0, materialDraft: null, draftError: null,
  selectedVolume: null, selectedMaterial: null, hiddenInstances: new Set<string>(), contextMenu: null,
  measureMode: false, measurements: [], pendingPoint: null, hoverSnap: null, hoverCandidates: [], surfaceCandidates: [],
}) satisfies Partial<AppState>;

const initialState = () => ({
  ...emptyDocument(),
  loading: false, error: null, documentBusy: false, decision: null, activeTreeTab: 'structure', meshOpacity: 1.0,
}) satisfies Partial<AppState>;

export const useAppStore = create<AppState>((set, get) => ({
  ...initialState(),

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  markDirty: () => set((state) => ({ dirty: true, editRevision: state.editRevision + 1 })),
  markSaved: (revision) => set((state) => revision === undefined || revision === state.editRevision ? { dirty: false } : {}),
  updateMaterialDraft: (changes, elements) => set((state) => {
    const original = state.materialDraft?.original ?? state.materials.find((m) => m.name === state.selectedMaterial);
    if (!original) return {};
    const material = { ...(state.materialDraft?.material ?? original), ...changes };
    const dependencies = elements ?? state.materialDraft?.elements ?? [];
    // A material that is not in the document yet stays a draft until Apply or Cancel.
    const unchanged = JSON.stringify(material) === JSON.stringify(original) && dependencies.length === 0
      && state.materials.some((m) => m.name === original.name);
    return { materialDraft: unchanged ? null : { original, material, elements: dependencies }, draftError: null };
  }),
  discardMaterialDraft: () => set((state) => ({
    materialDraft: null, draftError: null,
    error: state.error === state.draftError ? null : state.error,
    // Cancelling a material that was never applied leaves nothing to select.
    selectedMaterial: state.materials.some((m) => m.name === state.selectedMaterial) ? state.selectedMaterial : null,
  })),
  clearWarnings: () => set({ warnings: [] }),
  setSelectedVolume: (name) => {
    const state = get();
    if (state.documentBusy) return;
    const material = state.volumes.find((v) => v.name === name)?.material_ref ?? state.selectedMaterial;
    if (material === state.selectedMaterial || !state.materialDraft) {
      set({ selectedVolume: name, selectedMaterial: material }); return;
    }
    void runDocumentAction(async () => {
      if (!await resolveMaterialDraft()) return;
      const material = get().volumes.find((v) => v.name === name)?.material_ref ?? get().selectedMaterial;
      set({ selectedVolume: name, selectedMaterial: material });
    });
  },
  setSelectedMaterial: (name) => {
    const state = get();
    if (state.documentBusy || name === state.selectedMaterial) return;
    if (!state.materialDraft) { set({ selectedMaterial: name }); return; }
    void runDocumentAction(async () => {
      if (await resolveMaterialDraft()) set({ selectedMaterial: name });
    });
  },
  setActiveTreeTab: (tab) => set({ activeTreeTab: tab }),
  setMeshOpacity: (opacity) => set({ meshOpacity: opacity }),
  toggleInstanceVisibility: (instanceId) =>
    set((state) => {
      const next = new Set(state.hiddenInstances);
      if (next.has(instanceId)) {
        next.delete(instanceId);
      } else {
        next.add(instanceId);
      }
      return { hiddenInstances: next };
    }),
  openContextMenu: (x, y, items) => set({ contextMenu: { x, y, items } }),
  closeContextMenu: () => set({ contextMenu: null }),

  // Measurement actions
  setMeasureMode: (on) =>
    set(on ? { measureMode: true } : { measureMode: false, pendingPoint: null, hoverSnap: null, hoverCandidates: [], surfaceCandidates: [] }),
  placeMeasurePoint: (snap) =>
    set((state) => {
      if (!state.pendingPoint) {
        return { pendingPoint: snap };
      }
      const a = state.pendingPoint.position;
      const b = snap.position;
      const distance = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
      // A second click on point A would record a meaningless 0 mm measurement.
      if (distance < 1e-9) return {};
      const measurement: Measurement = {
        id: `m_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
        pointA: state.pendingPoint,
        pointB: snap,
        distance,
      };
      return { measurements: [...state.measurements, measurement], pendingPoint: null, surfaceCandidates: [] };
    }),
  cancelMeasure: () => {
    if (get().pendingPoint) set({ pendingPoint: null, surfaceCandidates: [] });
    else get().setMeasureMode(false);
  },
  clearMeasurements: () => set({ measurements: [], pendingPoint: null, surfaceCandidates: [] }),

  // Closes the document. A failed load calls this inside its document action,
  // which still owns the lock (documentBusy) and any dialog until it returns.
  reset: () => {
    clearAllGeometries();
    set({ ...emptyDocument(), error: null });
  },
}));
