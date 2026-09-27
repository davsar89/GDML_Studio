import type { MaterialInfo, MaterialComponent, NistMaterial, ElementInfo } from '../store/types';
import { useAppStore } from '../store';
import * as api from '../api/client';

/** A material with only a name and the density Geant4 requires. */
export function emptyMaterial(name: string): MaterialInfo {
  return {
    name, formula: null, z: null, state: null, density: { value: '1.0', unit: 'g/cm3' }, density_ref: null,
    temperature: null, pressure: null, mee: null, rl: null, al: null, properties: [],
    atom_value: null, atom_unit: null, atom_type: null, components: [],
  };
}

/** Prepare a material and its missing elements without changing the document. */
export async function prepareNistMaterial(nist: NistMaterial): Promise<{ material: MaterialInfo; elements: ElementInfo[] }> {
  const existing = useAppStore.getState().elements;
  const elements: ElementInfo[] = [];
  for (const name of new Set(nist.components.map((comp) => comp.ref))) {
    if (existing.some((el) => el.name === name)) continue;
    const { material: refNist } = await api.getNistMaterial(name);
    elements.push({
      name: refNist.name, formula: refNist.formula,
      z: refNist.z != null ? String(refNist.z) : null,
      atom_value: refNist.atom_value != null ? String(refNist.atom_value) : null,
      atom_unit: null, atom_type: null, fractions: [],
    });
  }

  // Build components
  const components: MaterialComponent[] = nist.components.map((c) =>
    c.type === 'Fraction'
      ? { Fraction: { n: String(c.n), ref_name: c.ref } }
      : { Composite: { n: String(c.n), ref_name: c.ref } },
  );

  const mat: MaterialInfo = {
    ...emptyMaterial(nist.name),
    formula: nist.formula,
    z: nist.z != null ? String(nist.z) : null,
    state: nist.state.toLowerCase() || null,
    density: { value: String(nist.density), unit: 'g/cm3' },
    atom_value: nist.atom_value != null ? String(nist.atom_value) : null,
    components,
  };

  return { material: mat, elements };
}
