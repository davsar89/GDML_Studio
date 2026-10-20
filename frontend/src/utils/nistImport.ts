import type { MaterialInfo, MaterialComponent, NistMaterial, ElementInfo } from '../store/types';
import { useAppStore } from '../store';
import * as api from '../api/client';

/** A material with only a name and the density Geant4 requires. */
export function emptyMaterial(name: string): MaterialInfo {
  return {
    name, formula: null, z: null, state: null, density: { value: '1.0', unit: 'g/cm3' }, density_ref: null,
    temperature: null, temperature_ref: null, pressure: null, pressure_ref: null, mee: null, mee_ref: null,
    rl: null, al: null, properties: [],
    atom_value: null, atom_unit: null, atom_type: null, components: [],
  };
}

/** Prepare a material and its missing elements without changing the document. */
export async function prepareNistMaterial(nist: NistMaterial): Promise<{ material: MaterialInfo; elements: ElementInfo[] }> {
  const existing = useAppStore.getState().elements;
  const elements: ElementInfo[] = [];
  const elementName = new Map<string, string>();
  for (const ref of new Set(nist.components.map((comp) => comp.ref))) {
    const { material: el } = await api.getNistMaterial(ref);
    const symbol = el.formula ?? ref;
    // Reuse the document's element for this Z. A new one takes its symbol, as in
    // Geant4's NIST element table, so it cannot collide with the G4_ material.
    const match = existing.find((e) => Number(e.z) === el.z) ?? existing.find((e) => e.name === symbol || e.name === ref);
    elementName.set(ref, match?.name ?? symbol);
    if (!match) {
      elements.push({
        name: symbol, formula: el.formula,
        z: el.z != null ? String(el.z) : null,
        atom_value: el.atom_value != null ? String(el.atom_value) : null,
        atom_unit: null, atom_type: null, fractions: [],
      });
    }
  }

  const components: MaterialComponent[] = nist.components.map((c) => {
    const part = { n: String(c.n), ref_name: elementName.get(c.ref)! };
    return c.type === 'Fraction' ? { Fraction: part } : { Composite: part };
  });

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
