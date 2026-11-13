export interface SceneNode {
  name: string;
  instance_id: string;
  volume_name: string;
  solid_name: string;
  material_name: string;
  color: string | null;
  density: number | null;
  position: [number, number, number];
  rotation: [number, number, number];
  scale: [number, number, number];
  is_world: boolean;
  children: SceneNode[];
}

export interface MeshData {
  positions: number[];
  normals: number[];
  indices: number[];
}

export interface DocumentSummary {
  filename: string;
  defines_count: number;
  positions_count: number;
  rotations_count: number;
  materials_count: number;
  elements_count: number;
  solids_count: number;
  volumes_count: number;
  meshes_count: number;
  world_ref: string;
  warnings: string[];
  /** Edits applied since the load or the last export. */
  unsaved: boolean;
  /** Imported from a CAD file: an import report is available. */
  imported: boolean;
}

export interface DefineValue {
  name: string;
  expression: string;
  evaluated: number | null;
  unit: string | null;
  kind: string;
}

export interface VolumeInfo {
  name: string;
  material_ref: string;
  solid_ref: string;
  physvols: { name: string | null; volume_ref: string }[];
  auxiliaries: { auxtype: string; auxvalue: string }[];
}

// ─── Material/Element types ─────────────────────────────────────────────────

export interface PropertyValue {
  value: string;
  unit: string | null;
}

export interface MaterialComponent {
  Fraction?: { n: string; ref_name: string };
  Composite?: { n: string; ref_name: string };
}

/** A `<fraction n=".." ref=".."/>` entry in an isotope-defined element. */
export interface Fraction {
  n: string;
  ref_name: string;
}

/** `<property name="RINDEX" ref="rindexMatrix"/>`: an optical property binding. */
export interface MaterialProperty {
  name: string;
  ref_name: string | null;
  values: string | null;
}

// MaterialInfo and ElementInfo are sent back to the backend, which stores them
// as sent. Every field the backend models must appear here, and edit paths must
// spread the original object rather than rebuild it field by field: anything
// missing is silently dropped from the user's GDML on export.
export interface MaterialInfo {
  name: string;
  formula: string | null;
  z: string | null;
  /** GDML `state="solid|liquid|gas"`. */
  state: string | null;
  density: PropertyValue | null;
  density_ref: string | null;
  temperature: PropertyValue | null;
  temperature_ref: string | null;
  pressure: PropertyValue | null;
  pressure_ref: string | null;
  /** `<MEE>` mean excitation energy. */
  mee: PropertyValue | null;
  mee_ref: string | null;
  /** `<RL>` radiation length. */
  rl: PropertyValue | null;
  /** `<AL>` absorption length. */
  al: PropertyValue | null;
  /** `<property>` bindings: the optical property table. */
  properties: MaterialProperty[];
  atom_value: string | null;
  atom_unit: string | null;
  atom_type: string | null;
  components: MaterialComponent[];
}

export interface ElementInfo {
  name: string;
  formula: string | null;
  z: string | null;
  atom_value: string | null;
  atom_unit: string | null;
  atom_type: string | null;
  /** Isotope composition; empty for elements defined by Z/atom alone. */
  fractions: Fraction[];
}

export interface NistComponent {
  type: 'Fraction' | 'Composite';
  ref: string;
  n: number;
}

export interface NistMaterial {
  name: string;
  density: number;
  state: string;
  category: string;
  z: number | null;
  atom_value: number | null;
  formula: string | null;
  components: NistComponent[];
}

// ─── CAD import (backend `cad::Options` and `cad::report`) ──────────────────

export interface PartMaterial {
  material: MaterialInfo;
  /** The elements it needs that the document lacks. */
  elements: ElementInfo[];
}

export interface ImportOptions {
  /** The length of one STL unit in mm: STL files carry no unit. */
  stl_unit_mm: number;
  /** The relative error of each mesh volume that STEP meshing aims for. */
  volume_target: number;
  /** Keep the STEP assemblies as `<assembly>` volumes, or place every part in the world. */
  hierarchy: 'keep' | 'flatten';
  /** One material for every part; null leaves them on the placeholder. */
  part_material: PartMaterial | null;
  /** Per-part choices, keyed by definition id. */
  overrides: Record<string, ImportOverride>;
}

export interface ImportOverride {
  representation: 'auto' | 'tessellated' | 'exclude';
  /** The chordal deviation of the part's mesh in mm, in place of the one the volume target leads to (STEP). */
  chord_mm?: number | null;
}

/** How far the running import got (backend `cad::Progress`). */
export interface ImportProgress { phase: string; done: number; total: number }

export interface ImportDefinition {
  id: string;
  /** The part's name in the source file. */
  label: string;
  /** Its logical volume; null when excluded. */
  volume: string | null;
  excluded: boolean;
  /** What the document holds for it: its exact GDML solid, or its mesh. */
  representation: 'exact' | 'tessellated';
  /** Its exact GDML solid ("tube", "box - 2 cut-outs"), proven equal to the CAD solid; null when it has none. */
  analytic: string | null;
  /** Why it has no exact GDML solid. */
  analytic_reason: string | null;
  /** The tolerance in mm of that proof. */
  epsilon_mm: number | null;
  /** Its placements in the world (assembly occurrences). */
  instances: number;
  /** The CAD kernel's properties of the solid (STEP only). */
  exact: {
    volume_mm3: number; area_mm2: number;
    /** Minimum x, y, z, then maximum, in the part's frame. */
    bounds_mm: number[];
    tolerance_mm: number; chord_mm: number;
  } | null;
  /** What the source or its reading flagged without preventing the import. */
  issues: string[];
  facets: number;
  vertices: number;
  shells: number;
  volume_mm3: number;
  area_mm2: number;
  bounds_mm: [[number, number, number], [number, number, number]];
  /** What repairing its source mesh changed. */
  repairs: { welded_vertices: number; weld_max_mm: number; dropped_facets: number; collapsed_edges: number; flipped_edges: number };
  reversed_facets: number;
}

export interface ImportReport {
  filename: string;
  format: 'stl' | 'step';
  bytes: number;
  options: ImportOptions;
  /** The material of parts that have none assigned yet. */
  placeholder_material: string;
  definitions: ImportDefinition[];
  /** Parts that could not be imported, and why. */
  rejected: { label: string; reason: string }[];
  /**
   * Placed solids of the source whose interiors overlap (STEP), each named by
   * its labels from the top of the assembly down, with its definition id when imported.
   */
  interferences: { paths: [string, string]; definitions: [string | null, string | null]; volume_mm3: number }[];
  /** Pairs of placed solids the interference check had no time for. */
  unchecked: number;
  /** Pairs of definition ids some placements of which have crossing meshes. */
  crossings: [string, string][];
  /** Pairs of definition ids of touching parts, one shown exact and one tessellated. */
  mixed_contacts: [string, string][];
  /** Facets of the included parts. */
  facets: number;
  gdml_bytes_estimate: number;
}

// ─── Measurement types ──────────────────────────────────────────────────────

export interface SnapPoint {
  position: [number, number, number];
  type: 'vertex' | 'edge' | 'face';
}

export interface Measurement {
  id: string;
  pointA: SnapPoint;
  pointB: SnapPoint;
  distance: number;
}
