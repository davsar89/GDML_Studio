import { describe, it, expect, vi } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';

vi.mock('../api/client', () => ({ getNistMaterials: vi.fn() }));
import * as api from '../api/client';
import type { NistMaterial } from '../store/types';
import { useNistSearch } from './useNistSearch';

describe('useNistSearch', () => {
  it('resolves out-of-order responses to the latest query', async () => {
    const answer = new Map<string, () => void>();
    vi.mocked(api.getNistMaterials).mockImplementation((search) => new Promise((resolve) => {
      answer.set(search!, () => resolve({ materials: [{ name: `G4_${search}` } as NistMaterial] }));
    }));
    const { result, rerender } = renderHook(({ search }) => useNistSearch(search), { initialProps: { search: 'WA' } });
    await waitFor(() => expect(answer.has('WA')).toBe(true));
    rerender({ search: 'WATER' });
    await waitFor(() => expect(answer.has('WATER')).toBe(true));
    await act(async () => answer.get('WATER')!());
    await act(async () => answer.get('WA')!());
    expect(result.current).toEqual({ results: [{ name: 'G4_WATER' }], loading: false });
  });
});
