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
  meshOpacity: number;
  hiddenVolumes: Set<string>;

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
  setMeshOpacity: (opacity: number) => void;
  toggleVolumeVisibility: (volumeName: string) => void;
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
  meshOpacity: 1.0,
  hiddenVolumes: new Set<string>(),

  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  setSummary: (summary) => set({ summary }),
  setMeshes: (meshes) => set({ meshes }),
  setSceneGraph: (graph) => set({ sceneGraph: graph }),
  setDefines: (defines) => set({ defines }),
  setVolumes: (volumes) => set({ volumes }),
  setMaterials: (materials) => set({ materials }),
  setElements: (elements) => set({ elements }),
  setSelectedVolume: (name) => set({ selectedVolume: name }),
  setSelectedMaterial: (name) => set({ selectedMaterial: name }),
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
      meshOpacity: 1.0,
      hiddenVolumes: new Set<string>(),
    });
  },
}));
