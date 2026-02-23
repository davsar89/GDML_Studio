import { create } from 'zustand';
import type { SceneNode, MeshData, DocumentSummary, DefineValue, VolumeInfo, MaterialInfo, ElementInfo } from './types';
import { clearAllGeometries } from '../components/Viewport/geometryCache';

interface AppState {
  loading: boolean;
  error: string | null;
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
  hiddenVolumes: Set<string>;
  contextMenu: {
    x: number;
    y: number;
    items: { label: string; action: () => void }[];
  } | null;

  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
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
  toggleVolumeVisibility: (volumeName: string) => void;
  openContextMenu: (x: number, y: number, items: { label: string; action: () => void }[]) => void;
  closeContextMenu: () => void;
  reset: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  loading: false,
  error: null,
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
  hiddenVolumes: new Set<string>(),
  contextMenu: null,

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
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
  toggleVolumeVisibility: (volumeName) =>
    set((state) => {
      const next = new Set(state.hiddenVolumes);
      if (next.has(volumeName)) {
        next.delete(volumeName);
      } else {
        next.add(volumeName);
      }
      return { hiddenVolumes: next };
    }),
  openContextMenu: (x, y, items) => set({ contextMenu: { x, y, items } }),
  closeContextMenu: () => set({ contextMenu: null }),
  reset: () => {
    clearAllGeometries();
    set({
      loading: false,
      error: null,
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
      hiddenVolumes: new Set<string>(),
      contextMenu: null,
    });
  },
}));
