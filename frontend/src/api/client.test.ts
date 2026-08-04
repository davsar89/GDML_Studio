import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('../components/Viewport/geometryCache', () => ({
  clearAllGeometries: vi.fn(),
}));

import * as api from './client';
import { useAppStore } from '../store';
import type { MaterialInfo } from '../store/types';

const material: MaterialInfo = {
  name: 'G4_WATER',
  formula: null,
  z: null,
  state: null,
  density: { value: '1.0', unit: 'g/cm3' },
  density_ref: null,
  temperature: null,
  pressure: null,
  mee: null,
  rl: null,
  al: null,
  properties: [],
  atom_value: null,
  atom_unit: null,
  atom_type: null,
  components: [],
};

function mockFetch(ok: boolean, body: unknown = { ok: true }) {
  return vi.fn().mockResolvedValue({
    ok,
    status: ok ? 200 : 500,
    statusText: ok ? 'OK' : 'Internal Server Error',
    json: async () => body,
  } as Response);
}

beforeEach(() => {
  useAppStore.getState().reset();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('unsaved-changes tracking', () => {
  it('starts clean', () => {
    expect(useAppStore.getState().dirty).toBe(false);
  });

  it.each([
    ['updateMaterial', () => api.updateMaterial('G4_WATER', material)],
    ['addMaterial', () => api.addMaterial(material)],
    ['deleteMaterial', () => api.deleteMaterial('G4_WATER')],
    ['deleteElement', () => api.deleteElement('H')],
    ['updateVolumeMaterialRef', () => api.updateVolumeMaterialRef('World', 'G4_AIR')],
  ])('%s marks the document dirty', async (_name, call) => {
    vi.stubGlobal('fetch', mockFetch(true));
    await call();
    expect(useAppStore.getState().dirty).toBe(true);
  });

  it.each([
    ['getMaterials', () => api.getMaterials()],
    ['getStructure', () => api.getStructure()],
    ['exportGdml', () => api.exportGdml()],
  ])('%s does not mark the document dirty', async (_name, call) => {
    vi.stubGlobal('fetch', mockFetch(true, { materials: [], elements: [], volumes: [], world_ref: '', gdml: '', filename: 'x' }));
    await call();
    expect(useAppStore.getState().dirty).toBe(false);
  });

  it('leaves the document clean when a mutation fails', async () => {
    // A rejected edit changed nothing, so claiming unsaved work would be a lie
    // -- and would keep the beforeunload prompt up for no reason.
    vi.stubGlobal('fetch', mockFetch(false, { error: 'nope' }));
    await expect(api.updateMaterial('G4_WATER', material)).rejects.toThrow('nope');
    expect(useAppStore.getState().dirty).toBe(false);
  });

  it('markSaved clears it and reset clears it', async () => {
    vi.stubGlobal('fetch', mockFetch(true));
    await api.updateMaterial('G4_WATER', material);
    expect(useAppStore.getState().dirty).toBe(true);

    useAppStore.getState().markSaved();
    expect(useAppStore.getState().dirty).toBe(false);

    useAppStore.getState().markDirty();
    useAppStore.getState().reset();
    expect(useAppStore.getState().dirty).toBe(false);
  });
});
