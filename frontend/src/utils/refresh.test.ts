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
import { refreshMaterialsAndMeshes } from './refresh';

const mocked = vi.mocked(api);

beforeEach(() => {
  vi.clearAllMocks();
  useAppStore.getState().reset();
  mocked.getMaterials.mockResolvedValue({ materials: [], elements: [] });
  mocked.getScene.mockResolvedValue({ scene_graph: null as never });
  mocked.getStructure.mockResolvedValue({ volumes: [], world_ref: 'World' });
});

describe('refreshMaterialsAndMeshes', () => {
  it('does not refetch the mesh payload', async () => {
    await refreshMaterialsAndMeshes();

    // update_material never re-tessellates -- tessellation runs only on upload
    // -- so pulling meshes here would re-upload every BufferGeometry to the GPU
    // (and ~16 MB of JSON on the largest sample) for no change at all.
    expect(mocked.getMeshes).not.toHaveBeenCalled();
    expect(mocked.getMaterials).toHaveBeenCalledTimes(1);
    expect(mocked.getScene).toHaveBeenCalledTimes(1);
  });

  it('surfaces a scene-refresh failure as a user-visible error', async () => {
    mocked.getScene.mockRejectedValue(new Error('backend exploded'));

    await refreshMaterialsAndMeshes();

    // Logging this to the console would leave the materials list updated while
    // the 3D view still showed pre-edit colours, with no sign anything failed.
    expect(useAppStore.getState().error).toContain('backend exploded');
  });
});
