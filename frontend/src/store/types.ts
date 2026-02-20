export interface SceneNode {
  name: string;
  volume_name: string;
  solid_name: string;
  material_name: string;
  color: string | null;
  density: number | null;
  position: [number, number, number];
  rotation: [number, number, number];
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
}

export interface DefineValue {
  name: string;
  expression: string;
  evaluated: number | null;
  unit: string | null;
  kind: string;
}

export interface SolidInfo {
  type: string;
  name: string;
  [key: string]: unknown;
}

export interface VolumeInfo {
  name: string;
  material_ref: string;
  solid_ref: string;
  physvols: { name: string | null; volume_ref: string }[];
  auxiliaries: { auxtype: string; auxvalue: string }[];
}
