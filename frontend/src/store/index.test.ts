import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../components/Viewport/geometryCache', () => ({
  clearAllGeometries: vi.fn(),
}));

import { useAppStore } from './index';
import { clearAllGeometries } from '../components/Viewport/geometryCache';
import type { MaterialInfo } from './types';

const material = (name: string): MaterialInfo => ({
  name,
  formula: null,
  z: null,
  state: null,
  density: { value: '1.0', unit: 'g/cm3' },
  density_ref: null,
  temperature: null, temperature_ref: null, pressure_ref: null, mee_ref: null,
  pressure: null,
  mee: null,
  rl: null,
  al: null,
  properties: [],
  atom_value: null,
  atom_unit: null,
  atom_type: null,
  components: [],
});

describe('store.reset', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('empties the document and disposes geometries', () => {
    useAppStore.setState({ materials: [material('G4_AIR')], warnings: ['something'] });
    const store = useAppStore.getState();
    store.setSelectedMaterial('G4_AIR');
    store.setError('boom');
    store.setMeasureMode(true);

    expect(useAppStore.getState().materials).toHaveLength(1);

    useAppStore.getState().reset();

    const after = useAppStore.getState();
    expect(after.materials).toEqual([]);
    expect(after.elements).toEqual([]);
    expect(after.volumes).toEqual([]);
    expect(after.defines).toEqual([]);
    expect(after.meshes).toEqual({});
    expect(after.sceneGraph).toBeNull();
    expect(after.summary).toBeNull();
    expect(after.selectedMaterial).toBeNull();
    expect(after.selectedVolume).toBeNull();
    expect(after.warnings).toEqual([]);
    expect(after.error).toBeNull();
    expect(after.loading).toBe(false);

    // Without this the GPU buffers of the discarded document are never freed.
    expect(clearAllGeometries).toHaveBeenCalledTimes(1);
  });
});

describe('measurement', () => {
  it('ignores a second point identical to the first', () => {
    const a = { position: [1, 2, 3] as [number, number, number], type: 'vertex' as const };
    const store = useAppStore.getState();
    store.placeMeasurePoint(a);
    store.placeMeasurePoint({ ...a, position: [1, 2, 3] });
    expect(useAppStore.getState().measurements).toEqual([]);
    expect(useAppStore.getState().pendingPoint).toBe(a);
    store.placeMeasurePoint({ position: [4, 6, 3], type: 'edge' });
    expect(useAppStore.getState().measurements.map((m) => m.distance)).toEqual([5]);
  });
});
