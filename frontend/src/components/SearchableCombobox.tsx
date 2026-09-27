import { Fragment, useState, useRef, useEffect } from 'react';
import type { MaterialInfo, NistMaterial } from '../store/types';
import { useNistSearch } from '../utils/useNistSearch';

interface SearchableComboboxProps {
  value: string;
  materials: MaterialInfo[];
  /** `nist` is set when the pick is a catalog material not yet in the document. */
  onChange: (materialName: string, nist?: NistMaterial) => void;
}

export default function SearchableCombobox({ value, materials, onChange }: SearchableComboboxProps) {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [activeIdx, setActiveIdx] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);

  const docMatches = materials.filter(
    (m) => !query || m.name.toUpperCase().includes(query.toUpperCase()),
  );
  // Catalog materials already in the document are listed once, under Document.
  const nistResults = useNistSearch(query || null).results
    .filter((n) => !materials.some((m) => m.name === n.name)).slice(0, 15);
  const allItems = [
    ...docMatches.map((m, i) => ({ name: m.name, nist: undefined, header: i === 0 ? 'Document' : undefined,
      detail: m.density ? `${m.density.value} ${m.density.unit || 'g/cm3'}` : undefined })),
    ...nistResults.map((n, i) => ({ name: n.name, nist: n, header: i === 0 ? 'NIST Database' : undefined,
      detail: `${n.density} g/cm3` })),
  ];

  const handleSelect = (item: typeof allItems[number]) => {
    setQuery('');
    setOpen(false);
    onChange(item.name, item.nist);
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
        value={open ? query : value}
        onChange={(e) => { setQuery(e.target.value); setOpen(true); setActiveIdx(-1); }}
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
          {allItems.map((item, i) => (
            <Fragment key={`${item.nist ? 'nist' : 'doc'}-${item.name}`}>
              {item.header && (
                <div style={{ fontSize: 9, color: '#556', padding: '2px 4px', fontWeight: 700, borderTop: item.nist ? '1px solid #0f3460' : undefined }}>
                  {item.header}
                </div>
              )}
              <div
                onMouseDown={() => handleSelect(item)}
                onMouseEnter={() => setActiveIdx(i)}
                style={{
                  fontSize: 10,
                  fontFamily: 'monospace',
                  color: '#b0b8c0',
                  padding: '2px 4px',
                  cursor: 'pointer',
                  background: activeIdx === i ? '#0f3460' : 'transparent',
                }}
              >
                <span style={{ color: '#56d6c8' }}>{item.name}</span>
                {item.nist?.formula && <span style={{ color: '#8899aa', marginLeft: 4 }}>{item.nist.formula}</span>}
                {item.detail && <span style={{ color: '#666', marginLeft: 6 }}>{item.detail}</span>}
                {item.nist && <span style={{ color: '#e94560', marginLeft: 4, fontSize: 9 }}>NIST</span>}
              </div>
            </Fragment>
          ))}
        </div>
      )}
    </div>
  );
}
