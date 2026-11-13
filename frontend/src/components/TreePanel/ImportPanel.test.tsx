import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, within, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../Viewport/geometryCache', () => ({ clearAllGeometries: vi.fn() }));
import { useAppStore } from '../../store';
import type { DocumentSummary, ImportDefinition, ImportOptions, ImportProgress, ImportReport, MaterialInfo, NistMaterial, VolumeInfo } from '../../store/types';
import { emptyMaterial } from '../../utils/nistImport';
import TreePanel from './TreePanel';
import Inspector from '../Inspector';
import Toolbar from '../Toolbar';
import DocumentDialog from '../DocumentDialog';

const summary: DocumentSummary = { filename: 'part.gdml', defines_count: 0, positions_count: 0, rotations_count: 0,
  materials_count: 1, elements_count: 0, solids_count: 3, volumes_count: 3, meshes_count: 3, world_ref: 'World',
  warnings: [], unsaved: true, imported: true };
const aluminium: NistMaterial = { name: 'G4_Al', formula: 'Al', density: 2.699, state: 'Solid', category: 'Elemental',
  z: 13, atom_value: 26.982, components: [] };
const definition = (id: string, label: string): ImportDefinition => ({
  id, label, volume: `${label}_lv`, excluded: false, representation: 'tessellated', instances: 1, exact: null, issues: [],
  facets: 12, vertices: 8, shells: 1,
  volume_mm3: 1000, area_mm2: 600, bounds_mm: [[0, 0, 0], [10, 10, 10]],
  repairs: { welded_vertices: 0, weld_max_mm: 0, dropped_facets: 0, collapsed_edges: 0, flipped_edges: 0 },
  reversed_facets: 0,
});
const volume = (name: string, material_ref: string): VolumeInfo => ({ name, material_ref, solid_ref: name, physvols: [], auxiliaries: [] });

let report: ImportReport;
let volumes: VolumeInfo[];
let materials: MaterialInfo[];
let requests: { path: string; options: ImportOptions; filename?: string | null; body?: unknown }[];
/** What the backend says of the running import; an import waits for Cancel while `holdImports` is set. */
let progress: ImportProgress | null;
let holdImports: boolean;
let release: () => void;

/** What the backend makes of `options`: every included part gets the part material. */
function importWith(options: ImportOptions) {
  const material = options.part_material?.material.name ?? 'CAD_UNASSIGNED';
  const definitions = report.definitions.map((d) => {
    const excluded = options.overrides[d.id]?.representation === 'exclude';
    return { ...d, excluded, volume: excluded ? null : `${d.label}_lv` };
  });
  report = { ...report, options, definitions };
  volumes = definitions.filter((d) => d.volume).map((d) => volume(d.volume!, material));
  materials = [emptyMaterial(material)];
}

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  localStorage.clear();
  requests = [];
  progress = { phase: 'mesh', done: 12, total: 90 };
  holdImports = false;
  report = {
    filename: 'part.stl', format: 'stl', bytes: 2_500_000, placeholder_material: 'CAD_UNASSIGNED',
    options: { stl_unit_mm: 1, volume_target: 1e-3, hierarchy: 'keep', part_material: null, overrides: {} },
    definitions: [
      { ...definition('0.0', 'bracket'), repairs: { welded_vertices: 3, weld_max_mm: 1e-6, dropped_facets: 2, collapsed_edges: 0, flipped_edges: 1 } },
      definition('1.0', 'plate'),
    ],
    rejected: [{ label: 'lid', reason: 'open surface: 4 edges have a facet on one side only (longest 2 mm)' }],
    interferences: [], unchecked: 0, crossings: [['0.0', '1.0']],
    facets: 24, gdml_bytes_estimate: 2_520,
  };
  importWith(report.options);
  vi.stubGlobal('fetch', vi.fn(async (url: string, init?: RequestInit) => {
    const [path, query] = url.split('?');
    const params = new URLSearchParams(query);
    let body: unknown;
    switch (path) {
      case '/api/import': {
        const options = JSON.parse(params.get('options')!);
        requests.push({ path, options, filename: params.get('filename'), body: init?.body });
        if (holdImports) {
          await new Promise<void>((resolve) => { release = resolve; });
          return new Response(JSON.stringify({ error: 'Import cancelled' }), { status: 409 });
        }
        importWith(options);
        body = summary; break;
      }
      case '/api/import/progress': body = progress; break;
      case '/api/import/cancel': release(); body = { cancelled: true }; break;
      case '/api/import/update': {
        const { options } = JSON.parse(String(init?.body));
        requests.push({ path, options });
        importWith(options);
        body = summary; break;
      }
      case '/api/document/import-report': body = report; break;
      case '/api/document/meshes': body = { meshes: {}, scene_graph: null, warnings: [] }; break;
      case '/api/document/defines': body = { defines: [] }; break;
      case '/api/document/structure': body = { volumes, world_ref: 'World' }; break;
      case '/api/document/materials': body = { materials, elements: [] }; break;
      case '/api/nist/materials': body = { materials: [aluminium] }; break;
      case '/api/nist/material': body = { material: aluminium }; break;
      default: throw new Error(`Unexpected request: ${path}`);
    }
    return new Response(JSON.stringify(body), { headers: { 'Content-Type': 'application/json' } });
  }));
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open'); };
});

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function mount() { render(<><Toolbar /><TreePanel /><Inspector /><DocumentDialog /></>); return userEvent.setup(); }

async function open(user: ReturnType<typeof userEvent.setup>, ...files: File[]) {
  const create = document.createElement.bind(document);
  let picker: HTMLInputElement | undefined;
  const spy = vi.spyOn(document, 'createElement').mockImplementation((tag, options) => {
    const element = create(tag, options);
    if (tag === 'input') picker = element as HTMLInputElement;
    return element;
  });
  await user.click(screen.getByRole('button', { name: 'Open File(s)' }));
  spy.mockRestore();
  fireEvent.change(picker!, { target: { files } });
}

const stl = (name: string) => new File(['solid s\nendsolid s\n'], name);

describe('CAD import', () => {
  it('imports one STL file as a new document and reports what became of each part', async () => {
    const user = mount();
    const file = stl('part.stl');
    await open(user, file);

    await screen.findByRole('button', { name: 'Import' });
    expect(requests).toEqual([{ path: '/api/import', filename: 'part.stl', body: file,
      options: { stl_unit_mm: 1, volume_target: 1e-3, hierarchy: 'keep', part_material: null, overrides: {} } }]);
    expect(screen.getByRole('button', { name: 'Import' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText('part.stl (STL, 2.5 MB)')).toBeInTheDocument();
    expect(screen.getByText('about 2.5 kB')).toBeInTheDocument();
    expect(screen.getByText('2 of 2 imported, 1 rejected')).toBeInTheDocument();
    expect(screen.getByText(/open surface: 4 edges/)).toBeInTheDocument();
    expect(screen.getByText('Repaired: 3 vertices welded (up to 0.000001 mm), 2 facets dropped, 1 edge flipped')).toBeInTheDocument();
    expect(screen.getByText('Mesh crosses plate')).toBeInTheDocument();
    expect(screen.queryByText(/Overlaps in the CAD file/)).toBeNull();

    // The unassigned parts lead to the material picker of the first one.
    await user.click(screen.getByRole('button', { name: '2 parts without a material (CAD_UNASSIGNED)' }));
    expect(useAppStore.getState().selectedVolume).toBe('bracket_lv');
    const inspector = within(screen.getByRole('complementary', { name: 'Inspector' }));
    expect(inspector.getByRole('combobox', { name: 'Material' })).toHaveValue('CAD_UNASSIGNED');
  });

  it('re-imports with edited options and starts the next import from them', async () => {
    const user = mount();
    await open(user, stl('part.stl'));
    await screen.findByRole('button', { name: 'Import' });

    await user.selectOptions(screen.getByRole('combobox', { name: 'STL unit' }), 'in');
    await user.click(screen.getByRole('checkbox', { name: 'Import plate' }));
    await user.type(screen.getByRole('combobox', { name: 'Part material' }), 'Al');
    await user.click(await screen.findByRole('option', { name: /G4_Al/ }));
    expect(screen.getByText(/replaces the material of every part/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Re-import' }));

    await waitFor(() => expect(screen.queryByRole('button', { name: 'Re-import' })).toBeNull());
    const { options } = requests[1];
    expect(requests[1].path).toBe('/api/import/update');
    expect(options.stl_unit_mm).toBe(25.4);
    expect(options.overrides).toEqual({ '1.0': { representation: 'exclude' } });
    expect(options.part_material).toMatchObject({ material: { name: 'G4_Al', z: '13', atom_value: '26.982',
      density: { value: '2.699', unit: 'g/cm3' }, state: 'solid' }, elements: [] });
    expect(screen.getByText('1 of 2 imported, 1 rejected')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Import plate' })).not.toBeChecked();
    expect(screen.getByRole('button', { name: /plate.*excluded/ })).toBeDisabled();

    // The imported document is unsaved, so opening the next file asks first.
    await open(user, stl('next.stl'));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Discard' }));
    await waitFor(() => expect(requests).toHaveLength(3));
    expect(requests[2].filename).toBe('next.stl');
    expect(requests[2].options).toMatchObject({ stl_unit_mm: 25.4, part_material: { material: { name: 'G4_Al' } }, overrides: {} });
  });

  it('re-imports a STEP file with its own options and lists the overlaps of its solids', async () => {
    const exact = { volume_mm3: 1001, area_mm2: 600, bounds_mm: [0, 0, 0, 10, 10, 10], tolerance_mm: 1e-7, chord_mm: 0.05 };
    report = {
      ...report, filename: 'arm.step', format: 'step', rejected: [], unchecked: 3,
      definitions: report.definitions.map((d) => ({ ...d, exact })),
      interferences: [
        { paths: ['arm / bracket', 'arm / plate'], definitions: ['0.0', '1.0'], volume_mm3: 2.5 },
        { paths: ['arm / bracket', 'arm / lid'], definitions: ['0.0', null], volume_mm3: 187.457 },
      ],
    };
    const user = mount();
    await open(user, new File(['ISO-10303-21;'], 'arm.step'));
    await screen.findByRole('button', { name: 'Import' });

    expect(screen.getByText('2 pairs of solids overlap, by up to 187.457 mm³')).toBeInTheDocument();
    expect(screen.getAllByRole('listitem').map((li) => li.textContent).filter((t) => t?.includes(' and '))).toEqual([
      'arm / bracket and arm / lid: 187.457 mm³', 'arm / bracket and arm / plate: 2.5 mm³']);
    expect(screen.getByText(/3 pairs of solids close enough to overlap were not checked/)).toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: 'STL unit' })).toBeNull();

    await user.selectOptions(screen.getByRole('combobox', { name: 'Mesh volume error' }), 'Fine: 0.01 %');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Assemblies' }), 'Flatten into the world');
    await user.type(screen.getByRole('spinbutton', { name: 'Chord of plate in mm' }), '0.01');
    await user.click(screen.getByRole('button', { name: 'Re-import' }));

    await waitFor(() => expect(requests).toHaveLength(2));
    expect(requests[1].options).toMatchObject({ volume_target: 1e-4, hierarchy: 'flatten',
      overrides: { '1.0': { representation: 'auto', chord_mm: 0.01 } } });
    await waitFor(() => expect(screen.getByRole('spinbutton', { name: 'Chord of plate in mm' })).toHaveValue(0.01));
    expect(screen.queryByRole('button', { name: 'Re-import' })).toBeNull();
  });

  it('shows how far an import got, and Cancel keeps the open document', async () => {
    const user = mount();
    await open(user, stl('part.stl'));
    await screen.findByRole('button', { name: 'Import' });
    const before = useAppStore.getState().importReport;

    holdImports = true;
    await open(user, stl('next.stl'));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Discard' }));
    expect(await screen.findByText('Importing: meshing 12/90', undefined, { timeout: 2000 })).toBeInTheDocument();
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByText(/^Importing/)).toBeNull());
    expect(useAppStore.getState()).toMatchObject({ importReport: before, error: null, documentBusy: false });
  });

  it('opens a CAD file only on its own', async () => {
    const user = mount();
    await open(user, stl('part.stl'), new File(['<gdml/>'], 'module.gdml'));
    expect(useAppStore.getState().error).toMatch(/on its own/);
    expect(requests).toEqual([]);
  });

  it('shows the Import tab only for an imported document', () => {
    useAppStore.setState({ activeTreeTab: 'import' });
    mount();
    expect(screen.queryByRole('button', { name: 'Import' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Structure' })).toHaveAttribute('aria-pressed', 'true');
  });
});
