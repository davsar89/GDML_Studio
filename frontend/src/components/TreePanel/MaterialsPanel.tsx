import { useState, useEffect, useCallback, useRef } from 'react';
import { useAppStore } from '../../store';
import type { MaterialInfo, ElementInfo, MaterialComponent, NistMaterial } from '../../store/types';
import * as api from '../../api/client';
import { prepareNistMaterial } from '../../utils/nistImport';
import { refreshMaterials } from '../../utils/refresh';
import { applyMaterialDraft, resolveMaterialDraft, runDocumentAction } from '../../utils/documentActions';

export default function MaterialsPanel() {
  const materials = useAppStore((s) => s.materials);
  const busy = useAppStore((s) => s.documentBusy || s.loading);
  const elements = useAppStore((s) => s.elements);
  const selectedMaterial = useAppStore((s) => s.selectedMaterial);
  const setSelectedMaterial = useAppStore((s) => s.setSelectedMaterial);

  if (materials.length === 0 && elements.length === 0) {
    return <div style={{ color: '#666', fontSize: 12 }}>No file loaded</div>;
  }

  return (
    <fieldset disabled={busy} style={{ display: 'flex', flexDirection: 'column', gap: 8, border: 0, padding: 0, margin: 0, minWidth: 0 }}>
      <MaterialsList
        materials={materials}
        selectedMaterial={selectedMaterial}
        onSelect={setSelectedMaterial}
      />
      {selectedMaterial && (
      <MaterialEditor
          materialName={selectedMaterial}
          materials={materials}
          elements={elements}
        />
      )}
      <ElementsList elements={elements} />
    </fieldset>
  );
}

// ─── Materials List ─────────────────────────────────────────────────────────

function MaterialsList({
  materials,
  selectedMaterial,
  onSelect,
}: {
  materials: MaterialInfo[];
  selectedMaterial: string | null;
  onSelect: (name: string | null) => void;
}) {
  const handleAdd = async () => {
    await runDocumentAction(async () => {
      if (!await resolveMaterialDraft()) return;
      const { materials, elements } = useAppStore.getState();
      const names = new Set([...materials, ...elements].map((item) => item.name));
      let name = 'Material_d1.0';
      for (let i = 2; names.has(name); i++) name = `Material_d1.0_${i}`;
      const newMat: MaterialInfo = {
        name,
        formula: null,
        z: null,
        state: null,
        density: { value: '1.0', unit: 'g/cm3' },
        density_ref: null,
        temperature: null,
        pressure: null,
        mee: null,
        rl: null,
        al: null,
        properties: [],
        atom_value: null,
        atom_unit: null,
        atom_type: null,
        components: [],
      };
      await api.addMaterial(newMat);
      await refreshMaterials();
      useAppStore.setState({ selectedMaterial: newMat.name });
    });
  };

  const handleDelete = async (name: string) => {
    await runDocumentAction(async () => {
      if (!await resolveMaterialDraft()) return;
      const target = selectedMaterial === name ? useAppStore.getState().selectedMaterial ?? name : name;
      if (!window.confirm(`Delete material "${target}"?`)) return;
      await api.deleteMaterial(target);
      await refreshMaterials();
      if (selectedMaterial === name) useAppStore.setState({ selectedMaterial: null });
    });
  };

  return (
    <div>
      <div style={{ ...sectionHeader, marginBottom: 4 }}>
        <span>Materials ({materials.length})</span>
        <button aria-label="Add material" onClick={handleAdd} style={smallBtn}>+</button>
      </div>
      <div style={{ maxHeight: 160, overflow: 'auto' }}>
        {materials.map((m) => (
          <div
            key={m.name}
            onClick={() => onSelect(m.name === selectedMaterial ? null : m.name)}
            onContextMenu={(e) => {
              e.preventDefault();
              useAppStore.getState().openContextMenu(e.clientX, e.clientY, [
                { label: 'Edit', action: () => onSelect(m.name) },
                { label: 'Delete', action: () => handleDelete(m.name) },
              ]);
            }}
            style={{
              padding: '2px 4px',
              cursor: 'pointer',
              fontSize: 11,
              fontFamily: 'monospace',
              background: m.name === selectedMaterial ? '#0f3460' : 'transparent',
              color: m.name === selectedMaterial ? '#e94560' : '#b0b8c0',
              borderRadius: 2,
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {m.name}
              {m.density && (
                <span style={{ color: '#666', marginLeft: 6 }}>
                  {m.density.value} {m.density.unit || 'g/cm3'}
                </span>
              )}
            </span>
            <span
              onClick={(e) => { e.stopPropagation(); handleDelete(m.name); }}
              style={{ cursor: 'pointer', color: '#e94560', fontSize: 10, opacity: 0.6, flexShrink: 0, marginLeft: 4 }}
              title="Delete material"
            >
              x
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── Material Editor ────────────────────────────────────────────────────────

function MaterialEditor({ materialName, materials, elements }: {
  materialName: string; materials: MaterialInfo[]; elements: ElementInfo[];
}) {
  const draft = useAppStore((s) => s.materialDraft);
  const draftError = useAppStore((s) => s.draftError);
  const update = useAppStore((s) => s.updateMaterialDraft);
  const original = materials.find((m) => m.name === materialName);
  const material = draft?.original.name === materialName ? draft.material : original;
  if (!material) return null;
  const availableElements = [...elements, ...(draft?.elements ?? []).filter((el) => !elements.some((existing) => existing.name === el.name))];
  return (
    <div style={{ borderTop: '1px solid #0f3460', paddingTop: 6 }}>
      <div style={sectionHeader}>Edit: {materialName}</div>
      <FieldRow label="Name">
        <input aria-label="Material name" value={material.name} onChange={(e) => update({ name: e.target.value })} style={{ ...inputStyle, flex: 1, minWidth: 0 }} />
      </FieldRow>
      <MaterialFields material={material} onChange={update} />
      <ComponentsList material={material} materials={materials} elements={availableElements} onChange={update} />
      <NistMaterialPicker material={material} />
      {draftError && <div role="alert" style={{ color: '#ff9999', fontSize: 12, marginTop: 6 }}>{draftError}</div>}
      <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
        <button disabled={!draft} onClick={() => runDocumentAction(applyMaterialDraft)} style={smallBtn}>Apply</button>
        <button disabled={!draft} onClick={() => useAppStore.getState().discardMaterialDraft()} style={smallBtn}>Cancel</button>
      </div>
    </div>
  );
}

function MaterialFields({ material, onChange }: {
  material: MaterialInfo; onChange: (changes: Partial<MaterialInfo>) => void;
}) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 3, marginTop: 4 }}>
      <FieldRow label="Density">
        <input aria-label="Density" value={material.density?.value ?? ''}
          onChange={(e) => onChange({ density: { value: e.target.value, unit: material.density?.unit ?? 'g/cm3' }, density_ref: null })}
          style={{ ...inputStyle, width: 80 }} />
        <select aria-label="Density unit" value={material.density?.unit ?? 'g/cm3'}
          onChange={(e) => onChange({ density: { value: material.density?.value ?? '', unit: e.target.value }, density_ref: null })}
          style={{ ...inputStyle, width: 70 }}>
          <option value="g/cm3">g/cm3</option><option value="kg/m3">kg/m3</option><option value="mg/cm3">mg/cm3</option>
        </select>
      </FieldRow>
      <FieldRow label="Formula">
        <input aria-label="Formula" value={material.formula ?? ''} onChange={(e) => onChange({ formula: e.target.value || null })} style={{ ...inputStyle, flex: 1, minWidth: 0 }} />
      </FieldRow>
      <FieldRow label="Z">
        <input aria-label="Atomic number" value={material.z ?? ''} onChange={(e) => onChange({ z: e.target.value || null })} style={{ ...inputStyle, width: 50 }} />
      </FieldRow>
    </div>
  );
}

function FieldRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
      <span style={{ fontSize: 10, color: '#8899aa', width: 50, flexShrink: 0 }}>{label}</span>
      {children}
    </div>
  );
}

// ─── Components List ────────────────────────────────────────────────────────

function ComponentsList({
  material,
  materials,
  elements,
  onChange,
}: {
  material: MaterialInfo;
  materials: MaterialInfo[];
  elements: ElementInfo[];
  onChange: (changes: Partial<MaterialInfo>) => void;
}) {
  const [addingComp, setAddingComp] = useState(false);
  const [compType, setCompType] = useState<'Fraction' | 'Composite'>('Fraction');
  const [compRef, setCompRef] = useState('');
  const [compN, setCompN] = useState('');

  const handleAddComponent = () => {
    if (!compRef || !compN) return;
    const newComp: MaterialComponent =
      compType === 'Fraction'
        ? { Fraction: { n: compN, ref_name: compRef } }
        : { Composite: { n: compN, ref_name: compRef } };
    onChange({ components: [...material.components, newComp] });
    setAddingComp(false);
    setCompRef('');
    setCompN('');
  };

  return (
    <div style={{ marginTop: 4 }}>
      <div style={{ ...sectionHeader, fontSize: 10 }}>
        <span>Components ({material.components.length})</span>
        <button onClick={() => setAddingComp(!addingComp)} style={smallBtn}>+</button>
      </div>
      {material.components.map((c, i) => {
        const f = c.Fraction;
        const co = c.Composite;
        const type = f ? 'frac' : 'comp';
        const n = f ? f.n : co!.n;
        const ref = f ? f.ref_name : co!.ref_name;
        return (
          <div key={i} style={{ fontSize: 10, fontFamily: 'monospace', color: '#b0b8c0', display: 'flex', gap: 4, alignItems: 'center' }}>
            <span style={{ color: '#8899aa' }}>{type}</span>
            <span style={{ color: '#56d6c8' }}>{ref}</span>
            <span style={{ color: '#666' }}>n={n}</span>
            <button aria-label={`Remove component ${ref}`}
              onClick={() => onChange({ components: material.components.filter((_, index) => index !== i) })}
              style={{ background: 'none', border: 0, cursor: 'pointer', color: '#e94560', fontSize: 9, opacity: 0.6 }}
            >
              x
            </button>
          </div>
        );
      })}
      {addingComp && (
        <div style={{ display: 'flex', gap: 3, marginTop: 2, flexWrap: 'wrap' }}>
          <select value={compType} onChange={(e) => setCompType(e.target.value as 'Fraction' | 'Composite')} style={{ ...inputStyle, width: 70 }}>
            <option value="Fraction">Fraction</option>
            <option value="Composite">Composite</option>
          </select>
          <select value={compRef} onChange={(e) => setCompRef(e.target.value)} style={{ ...inputStyle, flex: 1 }}>
            <option value="">-- ref --</option>
            <optgroup label="Elements">
              {elements.map((el) => (
                <option key={`element-${el.name}`} value={el.name}>{el.name}</option>
              ))}
            </optgroup>
            <optgroup label="Materials">
              {materials
                .filter((candidate) => candidate.name !== material.name)
                .map((candidate) => (
                  <option key={`material-${candidate.name}`} value={candidate.name}>
                    {candidate.name}
                  </option>
                ))}
            </optgroup>
          </select>
          <input
            value={compN}
            onChange={(e) => setCompN(e.target.value)}
            placeholder="n"
            style={{ ...inputStyle, width: 40 }}
          />
          <button onClick={handleAddComponent} style={smallBtn}>Add</button>
        </div>
      )}
    </div>
  );
}

// ─── NIST Material Picker ───────────────────────────────────────────────────

function NistMaterialPicker({ material }: { material: MaterialInfo }) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('');
  const [results, setResults] = useState<NistMaterial[]>([]);
  const [loading, setLoading] = useState(false);

  const doSearch = useCallback(async (q: string, cat: string) => {
    setLoading(true);
    try {
      const data = await api.getNistMaterials(q, cat || undefined);
      setResults(data.materials);
    } catch {
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    // Debounce so we don't fire an API request on every keystroke.
    const t = setTimeout(() => doSearch(search, category), 250);
    return () => clearTimeout(t);
  }, [open, search, category, doSearch]);

  const handleApply = (nist: NistMaterial) => runDocumentAction(async () => {
    const prepared = await prepareNistMaterial(nist);
    // The catalog supplies composition and density, not these GDML properties.
    const { temperature, pressure, mee, rl, al, properties } = material;
    useAppStore.getState().updateMaterialDraft({
      ...prepared.material, temperature, pressure, mee, rl, al, properties,
    }, prepared.elements);
    setOpen(false);
  });

  return (
    <div style={{ marginTop: 4 }}>
      <button onClick={() => setOpen(!open)} style={{ ...smallBtn, fontSize: 10 }}>
        {open ? 'Close NIST Lookup' : 'NIST Material Lookup'}
      </button>
      {open && (
        <div style={{ marginTop: 4 }}>
          <div style={{ display: 'flex', gap: 3, marginBottom: 3 }}>
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search..."
              style={{ ...inputStyle, flex: 1 }}
              autoFocus
            />
            <select value={category} onChange={(e) => setCategory(e.target.value)} style={{ ...inputStyle, width: 90 }}>
              <option value="">All</option>
              <option value="Elemental">Elemental</option>
              <option value="Compound">Compound</option>
              <option value="HEP">HEP</option>
              <option value="Space">Space</option>
              <option value="Biochemical">Biochemical</option>
            </select>
          </div>
          <div style={{ maxHeight: 150, overflow: 'auto' }}>
            {loading && <div style={{ fontSize: 10, color: '#666' }}>Loading...</div>}
            {results.map((n) => (
              <div
                key={n.name}
                onClick={() => handleApply(n)}
                style={{
                  fontSize: 10,
                  fontFamily: 'monospace',
                  color: '#b0b8c0',
                  cursor: 'pointer',
                  padding: '1px 2px',
                  borderRadius: 2,
                }}
                onMouseOver={(e) => (e.currentTarget.style.background = '#0f3460')}
                onMouseOut={(e) => (e.currentTarget.style.background = 'transparent')}
                title={`${n.name}${n.formula ? ' (' + n.formula + ')' : ''} - ${n.density} g/cm3 (${n.state}, ${n.category})`}
              >
                <span style={{ color: '#56d6c8' }}>{n.name}</span>
                {n.formula && <span style={{ color: '#8899aa', marginLeft: 4 }}>{n.formula}</span>}
                <span style={{ color: '#666', marginLeft: 6 }}>{n.density} g/cm3</span>
                <span style={{ color: '#555', marginLeft: 4 }}>{n.state}</span>
              </div>
            ))}
            {!loading && results.length === 0 && (
              <div style={{ fontSize: 10, color: '#666' }}>No results</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ─── Elements List ──────────────────────────────────────────────────────────

function ElementsList({ elements }: { elements: ElementInfo[] }) {
  const [expanded, setExpanded] = useState(false);
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState('');
  const [newZ, setNewZ] = useState('');
  const [newAtom, setNewAtom] = useState('');
  const [elSuggestions, setElSuggestions] = useState<NistMaterial[]>([]);
  const [showElSuggestions, setShowElSuggestions] = useState(false);
  const elSearchTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleNameChange = (value: string) => {
    setNewName(value);
    if (elSearchTimeout.current) clearTimeout(elSearchTimeout.current);
    if (value.length >= 1) {
      elSearchTimeout.current = setTimeout(async () => {
        try {
          const data = await api.getNistMaterials(value, 'Elemental');
          setElSuggestions(data.materials.slice(0, 10));
          setShowElSuggestions(data.materials.length > 0);
        } catch {
          setElSuggestions([]);
          setShowElSuggestions(false);
        }
      }, 200);
    } else {
      setElSuggestions([]);
      setShowElSuggestions(false);
    }
  };

  const handleSelectSuggestion = (nist: NistMaterial) => {
    setNewName(nist.name);
    if (nist.z != null) setNewZ(String(nist.z));
    if (nist.atom_value != null) setNewAtom(String(nist.atom_value));
    setShowElSuggestions(false);
  };

  const handleAdd = async () => {
    if (!newName.trim()) return;
    await runDocumentAction(async () => {
      await api.addElement({
        name: newName.trim(),
        formula: null,
        z: newZ || null,
        atom_value: newAtom || null,
        atom_unit: null,
        atom_type: null,
        fractions: [],
      });
      await refreshMaterials();
      setAdding(false);
      setNewName('');
      setNewZ('');
      setNewAtom('');
      setElSuggestions([]);
      setShowElSuggestions(false);
    });
  };

  const handleDelete = async (name: string) => {
    await runDocumentAction(async () => {
      if (!window.confirm(`Delete element "${name}"?`)) return;
      await api.deleteElement(name);
      await refreshMaterials();
    });
  };

  return (
    <div style={{ borderTop: '1px solid #0f3460', paddingTop: 6 }}>
      <div style={sectionHeader}>
        <span
          onClick={() => setExpanded(!expanded)}
          style={{ cursor: 'pointer' }}
        >
          {expanded ? '\u25BC' : '\u25B6'} Elements ({elements.length})
        </span>
        <button onClick={() => { setAdding(!adding); setExpanded(true); }} style={smallBtn}>+</button>
      </div>
      {expanded && (
        <>
          {adding && (
            <div style={{ display: 'flex', gap: 3, marginBottom: 3, flexWrap: 'wrap', position: 'relative' }}>
              <div style={{ position: 'relative', width: 60 }}>
                <input
                  value={newName}
                  onChange={(e) => handleNameChange(e.target.value)}
                  onFocus={() => { if (elSuggestions.length > 0) setShowElSuggestions(true); }}
                  onBlur={() => setTimeout(() => setShowElSuggestions(false), 150)}
                  placeholder="Name"
                  style={{ ...inputStyle, width: '100%' }}
                  autoFocus
                />
                {showElSuggestions && (
                  <div style={{
                    position: 'absolute', top: '100%', left: 0, zIndex: 100,
                    background: '#1a1a2e', border: '1px solid #0f3460', borderRadius: 3,
                    maxHeight: 120, overflow: 'auto', width: 180,
                  }}>
                    {elSuggestions.map((s) => (
                      <div
                        key={s.name}
                        onMouseDown={() => handleSelectSuggestion(s)}
                        style={{
                          fontSize: 10, fontFamily: 'monospace', color: '#b0b8c0',
                          padding: '2px 4px', cursor: 'pointer',
                        }}
                        onMouseOver={(e) => (e.currentTarget.style.background = '#0f3460')}
                        onMouseOut={(e) => (e.currentTarget.style.background = 'transparent')}
                      >
                        <span style={{ color: '#56d6c8' }}>{s.name}</span>
                        {s.z != null && <span style={{ color: '#666', marginLeft: 4 }}>Z={s.z}</span>}
                        {s.atom_value != null && <span style={{ color: '#666', marginLeft: 4 }}>A={s.atom_value}</span>}
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <input value={newZ} onChange={(e) => setNewZ(e.target.value)} placeholder="Z" style={{ ...inputStyle, width: 30 }} />
              <input value={newAtom} onChange={(e) => setNewAtom(e.target.value)} placeholder="Atom" style={{ ...inputStyle, width: 50 }} />
              <button onClick={handleAdd} style={smallBtn}>Add</button>
            </div>
          )}
          <div style={{ maxHeight: 120, overflow: 'auto' }}>
            {elements.map((el) => (
              <div
                key={el.name}
                style={{
                  fontSize: 10,
                  fontFamily: 'monospace',
                  color: '#b0b8c0',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  padding: '1px 0',
                }}
              >
                <span>
                  <span style={{ color: '#56d6c8' }}>{el.name}</span>
                  {el.z && <span style={{ color: '#666', marginLeft: 4 }}>Z={el.z}</span>}
                  {el.atom_value && <span style={{ color: '#666', marginLeft: 4 }}>A={el.atom_value}</span>}
                </span>
                <span
                  onClick={() => handleDelete(el.name)}
                  style={{ cursor: 'pointer', color: '#e94560', fontSize: 9, opacity: 0.6 }}
                  title="Delete element"
                >
                  x
                </span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const sectionHeader: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  color: '#8899aa',
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
};

const smallBtn: React.CSSProperties = {
  background: '#0f3460',
  color: '#e0e0e0',
  border: '1px solid #1a1a4e',
  borderRadius: 3,
  padding: '1px 6px',
  cursor: 'pointer',
  fontSize: 11,
};

const inputStyle: React.CSSProperties = {
  background: '#1a1a2e',
  color: '#e0e0e0',
  border: '1px solid #0f3460',
  borderRadius: 3,
  padding: '2px 4px',
  fontSize: 11,
  fontFamily: 'monospace',
  outline: 'none',
};
