import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../components/Viewport/geometryCache', () => ({
  clearAllGeometries: vi.fn(),
}));
vi.mock('../api/client', () => ({
  getMaterials: vi.fn(),
  getScene: vi.fn(),
  getStructure: vi.fn(),
  getMeshes: vi.fn(),
}));

import * as api from '../api/client';
import { useAppStore } from '../store';
import { refreshMaterialsAndScene } from './refresh';
import type { ElementInfo, SceneNode, VolumeInfo } from '../store/types';

const mocked = vi.mocked(api);
const elements: ElementInfo[] = [{ name: 'H', formula: 'H', z: '1', atom_value: '1', atom_unit: null, atom_type: null, fractions: [] }];
const scene: SceneNode = { name: 'World', instance_id: '/World', volume_name: 'World', solid_name: 'B', material_name: 'NewMaterial',
  density: 2, color: null, position: [0, 0, 0], rotation: [0, 0, 0], is_world: true, children: [] };
const volumes: VolumeInfo[] = [{ name: 'World', material_ref: 'NewMaterial', solid_ref: 'B', physvols: [], auxiliaries: [] }];

beforeEach(() => {
  vi.clearAllMocks();
  useAppStore.getState().reset();
  mocked.getMaterials.mockResolvedValue({ materials: [], elements });
  mocked.getScene.mockResolvedValue({ scene_graph: scene });
  mocked.getStructure.mockResolvedValue({ volumes, world_ref: 'World' });
});

describe('refreshMaterialsAndScene', () => {
  it('updates visible data while retaining the existing mesh buffers', async () => {
    const mesh = { positions: [0, 0, 0, 1, 0, 0, 0, 1, 0], normals: [0, 0, 1, 0, 0, 1, 0, 0, 1], indices: [0, 1, 2] };
    useAppStore.getState().setMeshes({ B: mesh });
    await refreshMaterialsAndScene();
    expect(useAppStore.getState().elements).toEqual(elements);
    expect(useAppStore.getState().sceneGraph).toEqual(scene);
    expect(useAppStore.getState().volumes).toEqual(volumes);
    expect(useAppStore.getState().meshes.B).toBe(mesh);
    expect(mocked.getMeshes).not.toHaveBeenCalled();
  });

  it('surfaces a scene-refresh failure as a user-visible error', async () => {
    mocked.getScene.mockRejectedValue(new Error('backend exploded'));

    await refreshMaterialsAndScene();

    // Logging this to the console would leave the materials list updated while
    // the 3D view still showed pre-edit colours, with no sign anything failed.
    expect(useAppStore.getState().error).toContain('backend exploded');
  });
});
