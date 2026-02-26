import type { MaterialInfo, MaterialComponent, NistMaterial } from '../store/types';
import { useAppStore } from '../store';
import * as api from '../api/client';

/**
 * Import a NIST material into the document, auto-creating referenced elements.
 * Returns the constructed MaterialInfo.
 */
export async function importNistMaterial(nist: NistMaterial): Promise<MaterialInfo> {
  const store = useAppStore.getState();

  // Auto-create referenced elements
  for (const comp of nist.components) {
    const existingEl = store.elements.find((el) => el.name === comp.ref);
    if (!existingEl) {
      try {
        const { material: refNist } = await api.getNistMaterial(comp.ref);
        await api.addElement({
          name: refNist.name,
          formula: refNist.formula,
          z: refNist.z != null ? String(refNist.z) : null,
          atom_value: refNist.atom_value != null ? String(refNist.atom_value) : null,
        });
      } catch {
        // Element may already exist or ref not found — continue gracefully
      }
    }
  }

  // Build components
  const components: MaterialComponent[] = nist.components.map((c) =>
    c.type === 'Fraction'
      ? { Fraction: { n: String(c.n), ref_name: c.ref } }
      : { Composite: { n: String(c.n), ref_name: c.ref } },
  );

  const mat: MaterialInfo = {
    name: nist.name,
    formula: nist.formula,
    z: nist.z != null ? String(nist.z) : null,
    density: { value: String(nist.density), unit: 'g/cm3' },
    density_ref: null,
    temperature: null,
    pressure: null,
    atom_value: nist.atom_value != null ? String(nist.atom_value) : null,
    components,
  };

  return mat;
}
