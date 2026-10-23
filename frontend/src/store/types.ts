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
