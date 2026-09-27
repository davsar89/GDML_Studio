import { useEffect, useState } from 'react';
import * as api from '../api/client';
import type { NistMaterial } from '../store/types';

/**
 * NIST catalog entries matching `search` (null: no search), fetched once typing
 * pauses. A response is kept only while its query is still the current one, so
 * a slow answer to an older query never replaces a newer one. The previous
 * results stay visible while the next query loads.
 */
export function useNistSearch(search: string | null, category = '') {
  const [found, setFound] = useState<{ key: string; materials: NistMaterial[] } | null>(null);
  const key = `${category}\n${search}`;
  useEffect(() => {
    if (search === null) return;
    let current = true;
    const timer = setTimeout(() => {
      api.getNistMaterials(search, category || undefined)
        .then((data) => data.materials, () => [])
        .then((materials) => { if (current) setFound({ key: `${category}\n${search}`, materials }); });
    }, 200);
    return () => { current = false; clearTimeout(timer); };
  }, [search, category]);
  return {
    results: search === null ? [] : found?.materials ?? [],
    loading: search !== null && found?.key !== key,
  };
}
