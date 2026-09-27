import { Fragment, useState, useRef, useEffect, useId } from 'react';
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
  const listId = useId();
  const docMatches = materials.filter((m) => !query || m.name.toUpperCase().includes(query.toUpperCase()));
  // Catalog materials already in the document are listed once, under Document.
  const nistResults = useNistSearch(query || null).results
    .filter((n) => !materials.some((m) => m.name === n.name)).slice(0, 15);
  const allItems = [
    ...docMatches.map((m, i) => ({ name: m.name, nist: undefined, header: i === 0 ? 'Document' : undefined,
      detail: m.density ? `${m.density.value} ${m.density.unit || 'g/cm3'}` : undefined })),
    ...nistResults.map((n, i) => ({ name: n.name, nist: n, header: i === 0 ? 'NIST Database' : undefined,
      detail: `${n.density} g/cm3` })),
  ];

  const close = () => { setOpen(false); setQuery(''); setActiveIdx(-1); };
  const handleSelect = (item: typeof allItems[number]) => { close(); onChange(item.name, item.nist); };
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'Enter') { setOpen(true); e.preventDefault(); }
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
    } else if (e.key === 'Escape') close();
  };

  useEffect(() => {
    if (open && activeIdx >= 0) containerRef.current?.querySelectorAll('[role="option"]')[activeIdx]?.scrollIntoView({ block: 'nearest' });
  }, [activeIdx, open]);

  return (
    <div ref={containerRef} className="popover-anchor" onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) close(); }}>
      <label className="field">Material
        <input role="combobox" aria-expanded={open} aria-controls={listId} aria-autocomplete="list"
          aria-activedescendant={open && activeIdx >= 0 ? `${listId}-${activeIdx}` : undefined}
          value={open ? query : value} autoComplete="off"
          onChange={(e) => { setQuery(e.target.value); setOpen(true); setActiveIdx(-1); }}
          onFocus={() => { setOpen(true); setQuery(''); setActiveIdx(-1); }}
          onKeyDown={handleKeyDown} placeholder="Search materials..." />
      </label>
      {open && <div className="popover" role="listbox" id={listId} aria-label="Materials">
        {allItems.map((item, i) => (
          <Fragment key={`${item.nist ? 'nist' : 'doc'}-${item.name}`}>
            {item.header && <div className="popover-heading" role="presentation">{item.header}</div>}
            <div role="option" id={`${listId}-${i}`} aria-selected={activeIdx === i}
              onMouseDown={(e) => e.preventDefault()} onClick={() => handleSelect(item)} onMouseEnter={() => setActiveIdx(i)}>
              <span className="mono">{item.name}</span>{' '}
              <span className="muted">{item.nist?.formula && `${item.nist.formula} · `}{item.detail}{item.nist && ' · NIST'}</span>
            </div>
          </Fragment>
        ))}
        {allItems.length === 0 && <div className="popover-heading" role="presentation">No matching materials</div>}
      </div>}
    </div>
  );
}
