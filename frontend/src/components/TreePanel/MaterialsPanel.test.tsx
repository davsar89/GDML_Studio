import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, within, fireEvent, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../Viewport/geometryCache', () => ({ clearAllGeometries: vi.fn() }));
import * as api from '../../api/client';
import { useAppStore } from '../../store';
import type { MaterialInfo, ElementInfo, DocumentSummary, NistMaterial, SceneNode } from '../../store/types';
import MaterialsPanel from './MaterialsPanel';
import VolumeDetail from './VolumeDetail';
import Toolbar from '../Toolbar';
import DocumentDialog from '../DocumentDialog';

const mocked = vi.mocked(api);
const NativeURL = URL;
const makeMaterial = (name = 'M'): MaterialInfo => ({
  name, formula: null, z: null, state: 'liquid', density: { value: '1.0', unit: 'g/cm3' },
  density_ref: null, temperature: null, pressure: null, mee: { value: '75', unit: 'eV' },
  rl: null, al: null, properties: [{ name: 'RINDEX', ref_name: 'optics', values: null }],
  atom_value: null, atom_unit: null, atom_type: null, components: [],
});
const summary: DocumentSummary = { filename: 'original.gdml', defines_count: 0, positions_count: 0, rotations_count: 0,
  materials_count: 2, elements_count: 0, solids_count: 0, volumes_count: 0, meshes_count: 0, world_ref: 'W', warnings: [] };
const scene: SceneNode = { name: 'W', volume_name: 'W', instance_id: '/W', solid_name: '', material_name: 'M',
  color: null, density: 1, position: [0,0,0], rotation: [0,0,0], is_world: true, children: [] };
const nist: NistMaterial = { name: 'CatalogMaterial', formula: 'H', density: 0.5, state: 'Gas', category: 'HEP',
  z: null, atom_value: null, components: [{ type: 'Fraction', ref: 'G4_H', n: 1 }] };
let serverMaterials: MaterialInfo[];
let serverElements: ElementInfo[];
let exportedDensity: string | undefined;

beforeEach(() => {
  useAppStore.getState().reset();
  vi.resetAllMocks();
  serverMaterials = [makeMaterial(), makeMaterial('N')];
  serverElements = [];
  exportedDensity = undefined;
  useAppStore.setState({ summary, materials: serverMaterials, selectedMaterial: 'M' });
  // Keep the real client: serialization, HTTP errors and dirty tracking must
  // execute in production code, not be reproduced by an API mock.
  for (const name of ['updateMaterial', 'addMaterial', 'addElement', 'exportGdml', 'uploadFiles'] as const) vi.spyOn(api, name);
  vi.stubGlobal('fetch', vi.fn(async (url: string, init?: RequestInit) => {
    const path = url.split('?')[0];
    let body: unknown;
    switch (path) {
      case '/api/document/materials': body = { materials: serverMaterials, elements: serverElements }; break;
      case '/api/document/scene': body = { scene_graph: scene }; break;
      case '/api/document/structure': body = { volumes: [], world_ref: 'W' }; break;
      case '/api/document/meshes': body = { meshes: {}, scene_graph: scene }; break;
      case '/api/document/defines': body = { defines: [] }; break;
      case '/api/document/materials/update': {
        const request = JSON.parse(String(init?.body));
        serverMaterials = serverMaterials.map((m) => m.name === request.name ? request.material : m);
        serverElements.push(...(request.elements ?? []));
        body = { ok: true }; break;
      }
      case '/api/document/materials/add':
        serverMaterials.push(JSON.parse(String(init?.body)).material);
        body = { ok: true }; break;
      case '/api/document/materials/delete':
        serverMaterials = serverMaterials.filter((m) => m.name !== JSON.parse(String(init?.body)).name);
        body = { ok: true }; break;
      case '/api/document/elements/delete':
        serverElements = serverElements.filter((e) => e.name !== JSON.parse(String(init?.body)).name);
        body = { ok: true }; break;
      case '/api/document/export':
        exportedDensity = serverMaterials[0].density?.value;
        body = { gdml: `<gdml density="${exportedDensity}"/>`, filename: summary.filename }; break;
      case '/api/files/upload-multi': body = { ...summary, filename: JSON.parse(String(init?.body)).main_file }; break;
      case '/api/nist/materials': body = { materials: [nist] }; break;
      // Assigning a material always fails here, which makes a NIST pick half-done.
      case '/api/document/structure/material-ref': return new Response(JSON.stringify({ error: 'refused' }), { status: 400 });
      case '/api/nist/material': body = { material: { ...nist, name: 'G4_H', category: 'Elemental', z: 1, atom_value: 1.008, components: [] } }; break;
      default: throw new Error(`Unexpected request: ${path}`);
    }
    return new Response(JSON.stringify(body), { headers: { 'Content-Type': 'application/json' } });
  }));
  vi.stubGlobal('URL', class extends NativeURL {
    static createObjectURL = vi.fn(() => 'blob:test');
    static revokeObjectURL = vi.fn();
  });
  vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
  // jsdom does not implement modal opening/focus trapping. Browser QA covers the native behavior.
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function mount() { render(<><Toolbar /><MaterialsPanel /><DocumentDialog /></>); return userEvent.setup(); }
const densityInput = () => screen.getByRole('textbox', { name: 'Density' });
async function edit(user: ReturnType<typeof userEvent.setup>, value = '3.25') {
  await user.clear(densityInput()); await user.type(densityInput(), value);
}
async function choose(user: ReturnType<typeof userEvent.setup>, label: string) {
  const dialog = await screen.findByRole('dialog');
  await user.click(within(dialog).getByRole('button', { name: label }));
}
async function chooseFile(user: ReturnType<typeof userEvent.setup>) {
  const create = document.createElement.bind(document);
  let picker: HTMLInputElement | undefined;
  const spy = vi.spyOn(document, 'createElement').mockImplementation((tag, options) => {
    const element = create(tag, options);
    if (tag === 'input') picker = element as HTMLInputElement;
    return element;
  });
  await user.click(screen.getByRole('button', { name: 'Open File(s)' }));
  spy.mockRestore();
  if (!picker) throw new Error('File chooser was not created');
  fireEvent.change(picker, { target: { files: [{ name: 'next.gdml', text: async () => '<gdml/>' }] } });
}

describe('material drafts and document actions', () => {
  it('offers Add for a loaded document without materials', () => {
    useAppStore.setState({ materials: [], selectedMaterial: null });
    mount();
    expect(screen.getByRole('button', { name: 'Add material' })).toBeInTheDocument();
  });

  it('Add material stays local until Apply', async () => {
    serverMaterials = [makeMaterial('Material_1')];
    serverElements = [{ name: 'Material_2', formula: null, z: '1', atom_value: '1', atom_unit: null, atom_type: null, fractions: [] }];
    useAppStore.setState({ materials: serverMaterials, elements: serverElements, selectedMaterial: null });
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'Add material' }));
    expect(screen.getByRole('textbox', { name: 'Material name' })).toHaveValue('Material_3');
    expect(screen.getByText('Material_3')).toHaveTextContent('Material_3 unapplied');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByText('Material_3')).toBeNull();
    expect(useAppStore.getState().selectedMaterial).toBeNull();
    expect(mocked.addMaterial).not.toHaveBeenCalled();
    expect(serverMaterials.map((m) => m.name)).toEqual(['Material_1']);

    await user.click(screen.getByRole('button', { name: 'Add material' }));
    vi.mocked(fetch).mockResolvedValueOnce(new Response(JSON.stringify({ error: 'needs atomic number and mass' }), { status: 400 }));
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('needs atomic number and mass');
    await user.type(screen.getByRole('textbox', { name: 'Atomic number' }), '1');
    await user.type(screen.getByRole('textbox', { name: 'Atomic mass' }), '1.008');
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(mocked.addMaterial).toHaveBeenCalledTimes(2);
    expect(mocked.updateMaterial).not.toHaveBeenCalled();
    expect(serverMaterials.map((m) => m.name)).toEqual(['Material_1', 'Material_3']);
    expect(serverMaterials[1]).toMatchObject({ z: '1', atom_value: '1.008' });
  });

  it('keeps edits local until Apply and preserves unedited GDML properties', async () => {
    const user = mount(); await edit(user);
    expect(mocked.updateMaterial).not.toHaveBeenCalled();
    expect(useAppStore.getState().dirty).toBe(false);
    const close = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(close);
    expect(close.defaultPrevented).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(mocked.updateMaterial).toHaveBeenCalledTimes(1);
    expect(serverMaterials[0].density?.value).toBe('3.25');
    expect(serverMaterials[0].mee).toEqual({ value: '75', unit: 'eV' });
    expect(serverMaterials[0].properties[0].ref_name).toBe('optics');
    expect(useAppStore.getState().materialDraft).toBeNull();
    expect(useAppStore.getState().dirty).toBe(true);
  });

  it.each(['Apply', 'Discard', 'Cancel'])('resolves a just-typed edit before Save: %s', async (choice) => {
    const user = mount(); await edit(user);
    await user.click(screen.getByRole('button', { name: 'Save' }));
    expect(mocked.exportGdml).not.toHaveBeenCalled();
    await choose(user, choice);
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    if (choice === 'Cancel') {
      expect(mocked.exportGdml).not.toHaveBeenCalled();
      expect(densityInput()).toHaveValue('3.25');
    } else {
      expect(exportedDensity).toBe(choice === 'Apply' ? '3.25' : '1.0');
      expect(useAppStore.getState().dirty).toBe(false);
      expect(useAppStore.getState().materialDraft).toBeNull();
    }
  });

  it('retains invalid drafts and blocks the pending export when Apply fails', async () => {
    const user = mount(); await edit(user, '-1');
    vi.mocked(fetch).mockResolvedValueOnce(new Response(JSON.stringify({ error: 'Density must be positive' }), { status: 400 }));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await choose(user, 'Apply');
    expect(await screen.findByRole('alert')).toHaveTextContent('Density must be positive');
    expect(mocked.exportGdml).not.toHaveBeenCalled();
    expect(densityInput()).toHaveValue('-1');
    expect(serverMaterials[0].density?.value).toBe('1.0');
    expect(useAppStore.getState().dirty).toBe(false);
  });

  it('blocks conflicting controls until an Apply request finishes', async () => {
    useAppStore.setState({ materials: [
      { ...makeMaterial(), components: [{ Fraction: { n: '1', ref_name: 'N' } }] }, makeMaterial('N'),
    ] });
    const user = mount(); await edit(user);
    let finish!: (value: { ok: boolean }) => void;
    mocked.updateMaterial.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(densityInput()).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Open File(s)' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    const remove = screen.getByRole('button', { name: 'Remove component N' });
    expect(remove).toBeDisabled();
    await user.click(remove);
    expect(useAppStore.getState().materialDraft?.material.components).toHaveLength(1);
    await act(async () => finish({ ok: true }));
    await waitFor(() => expect(densityInput()).not.toBeDisabled());
  });

  it('Cancel restores the applied value without discarding earlier unsaved work', async () => {
    const user = mount(); act(() => useAppStore.getState().markDirty());
    await edit(user);
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(densityInput()).toHaveValue('1.0');
    expect(mocked.updateMaterial).not.toHaveBeenCalled();
    expect(useAppStore.getState().dirty).toBe(true);
  });

  it('guards material selection and handles Escape as Cancel', async () => {
    const user = mount(); await edit(user);
    await user.click(screen.getByText('N', { exact: true }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    fireEvent(dialog, new Event('cancel', { cancelable: true }));
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(useAppStore.getState().selectedMaterial).toBe('M');
    expect(densityInput()).toHaveValue('3.25');
    await user.click(screen.getByText('N', { exact: true })); await choose(user, 'Discard');
    expect(useAppStore.getState().selectedMaterial).toBe('N');
    expect(densityInput()).toHaveValue('1.0');
  });

  it('stages NIST elements and cancels without creating any backend records', async () => {
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'NIST Material Lookup' }));
    await user.click(await screen.findByText('CatalogMaterial', { exact: true }));
    await waitFor(() => expect(screen.getByRole('textbox', { name: 'Material name' })).toHaveValue('CatalogMaterial'));
    expect(useAppStore.getState().materialDraft?.elements[0].name).toBe('H');
    expect(screen.getByText('mass 1')).toBeInTheDocument();
    expect(mocked.addElement).not.toHaveBeenCalled(); expect(mocked.updateMaterial).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(useAppStore.getState().materialDraft).toBeNull(); expect(useAppStore.getState().elements).toEqual([]);
  });

  it('starts a fresh NIST search per material', async () => {
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'NIST Material Lookup' }));
    await user.type(screen.getByPlaceholderText('Search...'), 'water');
    await user.click(screen.getByText('N', { exact: true }));
    await user.click(screen.getByRole('button', { name: 'NIST Material Lookup' }));
    expect(screen.getByPlaceholderText('Search...')).toHaveValue('');
  });

  it.each(['Save', 'Discard', 'Cancel'])('protects applied unsaved changes on file replacement: %s', async (choice) => {
    const user = mount(); act(() => useAppStore.getState().markDirty());
    await chooseFile(user);
    expect(mocked.uploadFiles).not.toHaveBeenCalled();
    await choose(user, choice);
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(mocked.exportGdml).toHaveBeenCalledTimes(choice === 'Save' ? 1 : 0);
    expect(mocked.uploadFiles).toHaveBeenCalledTimes(choice === 'Cancel' ? 0 : 1);
    expect(useAppStore.getState().summary?.filename).toBe(choice === 'Cancel' ? 'original.gdml' : 'next.gdml');
  });

  it('applies the NIST material and its dependencies in one request', async () => {
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'NIST Material Lookup' }));
    await user.click(await screen.findByText('CatalogMaterial', { exact: true }));
    await waitFor(() => expect(screen.getByRole('textbox', { name: 'Material name' })).toHaveValue('CatalogMaterial'));
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(mocked.updateMaterial).toHaveBeenCalledTimes(1);
    expect(serverMaterials[0].name).toBe('CatalogMaterial');
    expect(serverMaterials[0].state).toBe('gas');
    expect(serverElements.map((e) => e.name)).toEqual(['H']);
    expect(serverMaterials[0].components).toEqual([{ Fraction: { n: '1', ref_name: 'H' } }]);
    expect(mocked.addElement).not.toHaveBeenCalled();
    expect(serverMaterials[0].properties[0].ref_name).toBe('optics');
  });

  it('reuses an existing element with the same Z', async () => {
    serverElements = [{ name: 'Hydrogen', formula: null, z: '1', atom_value: '1.008', atom_unit: null, atom_type: null, fractions: [] }];
    useAppStore.setState({ elements: serverElements });
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'NIST Material Lookup' }));
    await user.click(await screen.findByText('CatalogMaterial', { exact: true }));
    await waitFor(() => expect(screen.getByRole('textbox', { name: 'Material name' })).toHaveValue('CatalogMaterial'));
    await user.click(screen.getByRole('button', { name: 'Apply' }));
    expect(serverElements.map((e) => e.name)).toEqual(['Hydrogen']);
    expect(serverMaterials[0].components).toEqual([{ Fraction: { n: '1', ref_name: 'Hydrogen' } }]);
  });

  it('keeps the old document when its replacement upload is rejected', async () => {
    const user = mount(); act(() => useAppStore.getState().markDirty());
    vi.mocked(fetch).mockResolvedValueOnce(new Response(JSON.stringify({ error: 'Invalid XML' }), { status: 400 }));
    await chooseFile(user); await choose(user, 'Discard');
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(useAppStore.getState().summary?.filename).toBe('original.gdml');
    expect(useAppStore.getState().dirty).toBe(true);
    expect(useAppStore.getState().materials).toEqual(serverMaterials);
  });

  it('aborts replacement on export failure and preserves the applied document', async () => {
    const user = mount(); act(() => useAppStore.getState().markDirty());
    vi.mocked(fetch).mockRejectedValueOnce(new Error('Export failed'));
    await chooseFile(user); await choose(user, 'Save');
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(mocked.uploadFiles).not.toHaveBeenCalled();
    expect(useAppStore.getState().dirty).toBe(true);
  });

  it('cancelled Save As leaves the draft untouched and does not Apply or export', async () => {
    const user = mount(); await edit(user);
    await user.click(screen.getByRole('button', { name: 'Save As' }));
    await choose(user, 'Cancel');
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(mocked.exportGdml).not.toHaveBeenCalled(); expect(mocked.updateMaterial).not.toHaveBeenCalled();
    expect(densityInput()).toHaveValue('3.25');
  });

  it('Save As appends .gdml and the next Save reuses it', async () => {
    const downloads: string[] = [];
    vi.mocked(HTMLAnchorElement.prototype.click).mockImplementation(function (this: HTMLAnchorElement) { downloads.push(this.download); });
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'Save As' }));
    const name = await screen.findByRole('textbox', { name: 'File name' });
    expect(name).toHaveValue('original.gdml');
    await user.clear(name); await user.type(name, 'copy{Enter}');
    await waitFor(() => expect(downloads).toEqual(['copy.gdml']));
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(downloads).toEqual(['copy.gdml', 'copy.gdml']));
    expect(useAppStore.getState().summary?.filename).toBe('copy.gdml');
  });

  it('returns focus to the control that started an action once it finishes', async () => {
    let finish!: (value: { gdml: string; filename: string }) => void;
    mocked.exportGdml.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'Save As' }));
    await user.type(await screen.findByRole('textbox', { name: 'File name' }), '{Enter}');
    // A few frames pass after the dialog closes while the export still holds the lock.
    await act(() => new Promise((resolve) => setTimeout(resolve, 100)));
    await act(async () => finish({ gdml: '<gdml/>', filename: 'original.gdml' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Save As' })).toHaveFocus());
  });

  it('asks in-app before deleting', async () => {
    serverElements = [{ name: 'H', formula: null, z: '1', atom_value: '1.008', atom_unit: null, atom_type: null, fractions: [] }];
    useAppStore.setState({ elements: serverElements });
    const user = mount();
    await user.click(screen.getAllByTitle('Delete material')[1]);
    expect(within(await screen.findByRole('dialog')).queryByRole('button', { name: 'Discard' })).toBeNull();
    await choose(user, 'Cancel');
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(serverMaterials.map((m) => m.name)).toEqual(['M', 'N']);
    await user.click(screen.getAllByTitle('Delete material')[1]); await choose(user, 'Delete');
    await waitFor(() => expect(useAppStore.getState().materials.map((m) => m.name)).toEqual(['M']));
    await user.click(screen.getByText(/Elements \(1\)/));
    await user.click(screen.getByTitle('Delete element')); await choose(user, 'Delete');
    await waitFor(() => expect(useAppStore.getState().elements).toEqual([]));
  });

  it('reaches element suggestions from the keyboard and adds only complete elements', async () => {
    const user = mount();
    await user.click(screen.getByRole('button', { name: 'Add element' }));
    await user.type(screen.getByPlaceholderText('Name'), 'Cat');
    await screen.findByRole('button', { name: /CatalogMaterial/ });
    await user.tab(); await user.keyboard('{Enter}');
    expect(screen.getByPlaceholderText('Name')).toHaveValue('CatalogMaterial');
    const add = screen.getByRole('button', { name: 'Add' });
    expect(add).toBeDisabled();
    await user.type(screen.getByPlaceholderText('Z'), '1'); await user.type(screen.getByPlaceholderText('Atom'), '1.008');
    expect(add).toBeEnabled();
  });

  it('does not clear a newer applied revision when an older export finishes', async () => {
    const user = mount(); act(() => useAppStore.getState().markDirty());
    let finish!: (value: { gdml: string; filename: string }) => void;
    mocked.exportGdml.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await act(async () => {
      useAppStore.getState().markDirty(); finish({ gdml: '<gdml/>', filename: 'original.gdml' });
    });
    expect(useAppStore.getState().dirty).toBe(true);
  });
});

describe('volume material picker', () => {
  const pick = () => {
    useAppStore.setState({ volumes: [{ name: 'W', material_ref: 'M', solid_ref: 'S', physvols: [], auxiliaries: [] }],
      selectedVolume: 'W', materials: [...serverMaterials] });
    render(<VolumeDetail />);
    return userEvent.setup();
  };

  it('re-picking the current material sends no request', async () => {
    const user = pick();
    await user.click(screen.getByPlaceholderText('Search materials...'));
    fireEvent.mouseDown(screen.getByText('M', { exact: true }));
    await waitFor(() => expect(useAppStore.getState().documentBusy).toBe(false));
    expect(fetch).not.toHaveBeenCalled();
    expect(useAppStore.getState().dirty).toBe(false);
  });

  it('refreshes the materials after a half-done NIST pick', async () => {
    const user = pick();
    await user.type(screen.getByPlaceholderText('Search materials...'), 'Cat');
    fireEvent.mouseDown(await screen.findByText('CatalogMaterial'));
    await waitFor(() => expect(useAppStore.getState().error).toBe('refused'));
    await waitFor(() => expect(useAppStore.getState().materials.map((m) => m.name)).toContain('CatalogMaterial'));
  });
});

