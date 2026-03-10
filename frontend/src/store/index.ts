import { create } from 'zustand';
import type { SceneNode, MeshData, DocumentSummary, DefineValue, VolumeInfo, MaterialInfo, ElementInfo, SnapPoint, Measurement } from './types';
import { clearAllGeometries } from '../components/Viewport/geometryCache';

interface AppState {
  loading: boolean;
  error: string | null;
  warnings: string[];
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
  setWarnings: (warnings: string[]) => void;
  clearWarnings: () => void;
  setSummary: (summary: DocumentSummary) => void;
  setMeshes: (meshes: Record<string, MeshData>) => void;
  setSceneGraph: (graph: SceneNode) => void;
  setDefines: (defines: DefineValue[]) => void;
  setVolumes: (volumes: VolumeInfo[]) => void;
  setMaterials: (materials: MaterialInfo[]) => void;
  setElements: (elements: ElementInfo[]) => void;
  setSelectedVolume: (name: string | null) => void;
  setSelectedMaterial: (name: string | null) => void;
  setActiveTreeTab: (tab: 'structure' | 'defines' | 'materials') => void;
  setMeshOpacity: (opacity: number) => void;
  toggleInstanceVisibility: (instanceId: string) => void;
  openContextMenu: (x: number, y: number, items: { label: string; action: () => void }[]) => void;
  closeContextMenu: () => void;

  // Measurement actions
  setMeasureMode: (on: boolean) => void;
  setHoverSnap: (snap: SnapPoint | null) => void;
  setHoverCandidates: (candidates: SnapPoint[]) => void;
  setSurfaceCandidates: (candidates: SnapPoint[]) => void;
  placeMeasurePoint: (snap: SnapPoint) => void;
  cancelMeasure: () => void;
  clearMeasurements: () => void;

  reset: () => void;
}
export const useAppStore = create<AppState>((set) => ({
  loading: false,
  error: null,
  warnings: [],
  summary: null,
  meshes: {},
  sceneGraph: null,
  defines: [],
  volumes: [],
  materials: [],
  elements: [],
  selectedVolume: null,
  selectedMaterial: null,
  activeTreeTab: 'structure',
  meshOpacity: 1.0,
  hiddenInstances: new Set<string>(),
  contextMenu: null,

  // Measurement defaults
  measureMode: false,
  measurements: [],
  pendingPoint: null,
  hoverSnap: null,
  hoverCandidates: [],
  surfaceCandidates: [],

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  setWarnings: (warnings) => set({ warnings }),
  clearWarnings: () => set({ warnings: [] }),
  setSummary: (summary) => set({ summary }),
  setMeshes: (meshes) => set({ meshes }),
  setSceneGraph: (graph) => set({ sceneGraph: graph }),
  setDefines: (defines) => set({ defines }),
  setVolumes: (volumes) => set({ volumes }),
  setMaterials: (materials) => set({ materials }),
  setElements: (elements) => set({ elements }),
  setSelectedVolume: (name) =>
    set((state) => {
      if (!name) return { selectedVolume: null };
      const vol = state.volumes.find((v) => v.name === name);
      return {
        selectedVolume: name,
        selectedMaterial: vol?.material_ref ?? state.selectedMaterial,
      };
    }),
  setSelectedMaterial: (name) => set({ selectedMaterial: name }),
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
  setHoverSnap: (snap) => set({ hoverSnap: snap }),
  setHoverCandidates: (candidates) => set({ hoverCandidates: candidates }),
  setSurfaceCandidates: (candidates) => set({ surfaceCandidates: candidates }),
  placeMeasurePoint: (snap) =>
    set((state) => {
      if (!state.pendingPoint) {
        return { pendingPoint: snap };
      }
      const a = state.pendingPoint.position;
      const b = snap.position;
      const dx = b[0] - a[0], dy = b[1] - a[1], dz = b[2] - a[2];
      const distance = Math.sqrt(dx * dx + dy * dy + dz * dz);
      const measurement: Measurement = {
        id: `m_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
        pointA: state.pendingPoint,
        pointB: snap,
        distance,
      };
      return { measurements: [...state.measurements, measurement], pendingPoint: null, surfaceCandidates: [] };
    }),
  cancelMeasure: () =>
    set((state) => {
      if (state.pendingPoint) return { pendingPoint: null, surfaceCandidates: [] };
      return { measureMode: false, pendingPoint: null, hoverSnap: null, surfaceCandidates: [] };
    }),
  clearMeasurements: () => set({ measurements: [], pendingPoint: null, surfaceCandidates: [] }),

  reset: () => {
    clearAllGeometries();
    set({
      loading: false,
      error: null,
      warnings: [],
      summary: null,
      meshes: {},
      sceneGraph: null,
      defines: [],
      volumes: [],
      materials: [],
      elements: [],
      selectedVolume: null,
      selectedMaterial: null,
      activeTreeTab: 'structure',
      meshOpacity: 1.0,
      hiddenInstances: new Set<string>(),
      contextMenu: null,
      measureMode: false,
      measurements: [],
      pendingPoint: null,
      hoverSnap: null,
      hoverCandidates: [],
      surfaceCandidates: [],
    });
  },
}));
