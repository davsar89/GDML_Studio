import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../Viewport/geometryCache', () => ({
  clearAllGeometries: vi.fn(),
}));
vi.mock('../../api/client', () => ({
  getMaterials: vi.fn(),
  getScene: vi.fn(),
  getStructure: vi.fn(),
  getMeshes: vi.fn(),
  updateMaterial: vi.fn(),
  addMaterial: vi.fn(),
  deleteMaterial: vi.fn(),
  addElement: vi.fn(),
  getNistMaterials: vi.fn(),
  getNistMaterial: vi.fn(),
}));

import * as api from '../../api/client';
import { useAppStore } from '../../store';
import type { MaterialInfo } from '../../store/types';
import MaterialsPanel from './MaterialsPanel';

const mocked = vi.mocked(api);

/** The debounce in MaterialFields, plus a margin. */
const DEBOUNCE_MS = 500;
const settle = () => new Promise((r) => setTimeout(r, DEBOUNCE_MS + 200));

function makeMaterial(density: string): MaterialInfo {
  return {
    name: 'G4_WATER',
    formula: null,
    z: null,
    state: 'liquid',
    density: { value: density, unit: 'g/cm3' },
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
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

function densityInput() {
  const row = screen.getByText('Density').parentElement;
  if (!row) throw new Error('Density row not found');
  return within(row).getByRole('textbox');
}

beforeEach(() => {
  vi.clearAllMocks();
  useAppStore.getState().reset();
  useAppStore.getState().setMaterials([makeMaterial('1')]);
  useAppStore.getState().setSelectedMaterial('G4_WATER');

  mocked.updateMaterial.mockResolvedValue({ ok: true });
  mocked.getMaterials.mockResolvedValue({ materials: [makeMaterial('1')], elements: [] });
  mocked.getScene.mockResolvedValue({ scene_graph: null as never });
  mocked.getStructure.mockResolvedValue({ volumes: [], world_ref: 'World' });
  mocked.getNistMaterials.mockResolvedValue({ materials: [] });
});

describe('MaterialFields density editing', () => {
  it('keeps text typed while a save is still in flight', async () => {
    const user = userEvent.setup();
    render(<MaterialsPanel />);

    // The refresh that follows the save is held open so we can type into the
    // window where the request is outstanding -- the 6.7 MB sample makes this
    // window hundreds of milliseconds wide in practice.
    const refresh = deferred<{ materials: MaterialInfo[]; elements: [] }>();
    mocked.getMaterials.mockReturnValueOnce(refresh.promise as never);

    await user.type(densityInput(), '2.3');
    expect(densityInput()).toHaveValue('12.3');

    await act(async () => {
      await settle();
    });
    expect(mocked.updateMaterial).toHaveBeenCalledTimes(1);

    // Still typing while the save round-trips.
    await user.type(densityInput(), '4');
    expect(densityInput()).toHaveValue('12.34');

    // The echo carries what was saved -- 12.3 -- not what is now on screen.
    await act(async () => {
      refresh.resolve({ materials: [makeMaterial('12.3')], elements: [] });
      await refresh.promise;
    });

    // Before the fix MaterialFields was keyed on the material's own values, so
    // this echo remounted it and re-initialised the input to "12.3", dropping
    // the 4 and firing a second POST from the unmount flush.
    expect(densityInput()).toHaveValue('12.34');
    expect(mocked.updateMaterial).toHaveBeenCalledTimes(1);
  });

  it('adopts a value written by something other than this component', async () => {
    const user = userEvent.setup();
    render(<MaterialsPanel />);

    await user.type(densityInput(), '2.3');
    expect(densityInput()).toHaveValue('12.3');

    // What the NIST picker does: replace density/formula/Z wholesale. The
    // incoming value differs from what we last sent, so it is not our echo and
    // the inputs have to re-sync or they would keep showing the stale number.
    await act(async () => {
      useAppStore.getState().setMaterials([makeMaterial('0.9998')]);
    });

    expect(densityInput()).toHaveValue('0.9998');
  });

  it('sends the whole material, not just the edited fields', async () => {
    const user = userEvent.setup();
    render(<MaterialsPanel />);

    await user.type(densityInput(), '2.3');
    await act(async () => {
      await settle();
    });

    // update_material replaces the stored material wholesale, so anything the
    // payload omits is erased from the exported GDML.
    const [name, sent] = mocked.updateMaterial.mock.calls[0];
    expect(name).toBe('G4_WATER');
    expect(sent.state).toBe('liquid');
    expect(sent.density).toEqual({ value: '12.3', unit: 'g/cm3' });
  });
});
