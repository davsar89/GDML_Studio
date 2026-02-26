import { useState, useRef, useEffect, useCallback } from 'react';
import type { MaterialInfo, NistMaterial } from '../store/types';
import * as api from '../api/client';

interface SearchableComboboxProps {
  value: string;
  materials: MaterialInfo[];
  onChange: (materialName: string, isNist: boolean, nistData?: NistMaterial) => void;
  inputRef?: React.RefObject<HTMLInputElement | null>;
}

export default function SearchableCombobox({ value, materials, onChange, inputRef }: SearchableComboboxProps) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [nistResults, setNistResults] = useState<NistMaterial[]>([]);
  const [activeIdx, setActiveIdx] = useState(-1);
  const searchTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const localInputRef = useRef<HTMLInputElement>(null);
  const effectiveRef = inputRef || localInputRef;

  const docMatches = materials.filter(
    (m) => !query || m.name.toUpperCase().includes(query.toUpperCase()),
  );

  const fetchNist = useCallback((q: string) => {
    if (searchTimeout.current) clearTimeout(searchTimeout.current);
    if (!q) {
      setNistResults([]);
      return;
    }
    searchTimeout.current = setTimeout(async () => {
      try {
        const data = await api.getNistMaterials(q);
        // Filter out materials already in the document
        const docNames = new Set(materials.map((m) => m.name));
        setNistResults(data.materials.filter((n) => !docNames.has(n.name)).slice(0, 15));
      } catch {
        setNistResults([]);
      }
    }, 200);
  }, [materials]);

  const handleInputChange = (val: string) => {
    setQuery(val);
    setOpen(true);
    setActiveIdx(-1);
    fetchNist(val);
  };

  const allItems: { label: string; isNist: boolean; nist?: NistMaterial; detail?: string }[] = [
    ...docMatches.map((m) => ({
      label: m.name,
      isNist: false,
      detail: m.density ? `${m.density.value} ${m.density.unit || 'g/cm3'}` : undefined,
    })),
    ...nistResults.map((n) => ({
      label: n.name,
      isNist: true,
      nist: n,
      detail: `${n.density} g/cm3 (NIST)`,
    })),
  ];

  const handleSelect = (item: typeof allItems[number]) => {
    setQuery('');
    setOpen(false);
    onChange(item.label, item.isNist, item.nist);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'Enter') {
        setOpen(true);
        e.preventDefault();
      }
      return;
    }
    if (e.key === 'ArrowDown') {
      setActiveIdx((prev) => Math.min(prev + 1, allItems.length - 1));
      e.preventDefault();
    } else if (e.key === 'ArrowUp') {
      setActiveIdx((prev) => Math.max(prev - 1, -1));
      e.preventDefault();
    } else if (e.key === 'Enter' && activeIdx >= 0 && activeIdx < allItems.length) {
      handleSelect(allItems[activeIdx]);
      e.preventDefault();
    } else if (e.key === 'Escape') {
      setOpen(false);
      setQuery('');
    }
  };

  // Close on click outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
        setQuery('');
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  return (
    <div ref={containerRef} style={{ position: 'relative', flex: 1 }}>
      <input
        ref={effectiveRef as React.RefObject<HTMLInputElement>}
        value={open ? query : value}
        onChange={(e) => handleInputChange(e.target.value)}
        onFocus={() => { setOpen(true); setQuery(''); }}
        onKeyDown={handleKeyDown}
        placeholder="Search materials..."
        style={{
          background: '#1a1a2e',
          color: '#e0e0e0',
          border: '1px solid #0f3460',
          borderRadius: 3,
          padding: '1px 4px',
          fontSize: 10,
          fontFamily: 'monospace',
          outline: 'none',
          width: '100%',
          boxSizing: 'border-box',
        }}
      />
      {open && allItems.length > 0 && (
        <div style={{
          position: 'absolute',
          top: '100%',
          left: 0,
          right: 0,
          zIndex: 200,
          background: '#1a1a2e',
          border: '1px solid #0f3460',
          borderRadius: 3,
          maxHeight: 200,
          overflow: 'auto',
        }}>
          {docMatches.length > 0 && (
            <div style={{ fontSize: 9, color: '#556', padding: '2px 4px', fontWeight: 700 }}>
              Document
            </div>
          )}
          {docMatches.map((m, i) => (
            <div
              key={m.name}
              onMouseDown={() => handleSelect(allItems[i])}
              style={{
                fontSize: 10,
                fontFamily: 'monospace',
                color: '#b0b8c0',
                padding: '2px 4px',
                cursor: 'pointer',
                background: activeIdx === i ? '#0f3460' : 'transparent',
              }}
              onMouseOver={(e) => { e.currentTarget.style.background = '#0f3460'; setActiveIdx(i); }}
              onMouseOut={(e) => { if (activeIdx !== i) e.currentTarget.style.background = 'transparent'; }}
            >
              <span style={{ color: '#56d6c8' }}>{m.name}</span>
              {m.density && (
                <span style={{ color: '#666', marginLeft: 6 }}>
                  {m.density.value} {m.density.unit || 'g/cm3'}
                </span>
              )}
            </div>
          ))}
          {nistResults.length > 0 && (
            <div style={{ fontSize: 9, color: '#556', padding: '2px 4px', fontWeight: 700, borderTop: '1px solid #0f3460' }}>
              NIST Database
            </div>
          )}
          {nistResults.map((n, j) => {
            const idx = docMatches.length + j;
            return (
              <div
                key={n.name}
                onMouseDown={() => handleSelect(allItems[idx])}
                style={{
                  fontSize: 10,
                  fontFamily: 'monospace',
                  color: '#b0b8c0',
                  padding: '2px 4px',
                  cursor: 'pointer',
                  background: activeIdx === idx ? '#0f3460' : 'transparent',
                }}
                onMouseOver={(e) => { e.currentTarget.style.background = '#0f3460'; setActiveIdx(idx); }}
                onMouseOut={(e) => { if (activeIdx !== idx) e.currentTarget.style.background = 'transparent'; }}
              >
                <span style={{ color: '#56d6c8' }}>{n.name}</span>
                {n.formula && <span style={{ color: '#8899aa', marginLeft: 4 }}>{n.formula}</span>}
                <span style={{ color: '#666', marginLeft: 6 }}>{n.density} g/cm3</span>
                <span style={{ color: '#e94560', marginLeft: 4, fontSize: 9 }}>NIST</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
