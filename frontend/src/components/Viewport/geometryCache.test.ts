import { describe, it, expect, vi } from 'vitest';
import { sharedMaterial, clearAllGeometries } from './geometryCache';

describe('sharedMaterial', () => {
  it('shares one material per look, draws daughters nearer and frees on clear', () => {
    const mother = sharedMaterial('#336699', false, 0.5, 1);
    const daughter = sharedMaterial('#336699', false, 0.5, 2);
    expect(sharedMaterial('#336699', false, 0.5, 1)).toBe(mother);
    expect(mother).toMatchObject({ transparent: true, depthWrite: false, opacity: 0.5, polygonOffset: true });
    expect(daughter.polygonOffsetFactor).toBeLessThan(mother.polygonOffsetFactor);
    expect(daughter.polygonOffsetUnits).toBeLessThan(mother.polygonOffsetUnits);

    const dispose = vi.spyOn(mother, 'dispose');
    clearAllGeometries();
    expect(dispose).toHaveBeenCalledTimes(1);
    expect(sharedMaterial('#336699', false, 0.5, 1)).not.toBe(mother);
  });
});
