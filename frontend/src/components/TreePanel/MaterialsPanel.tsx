import { useState, useEffect, useCallback, useRef } from 'react';
import { useAppStore } from '../../store';
import type { MaterialInfo, ElementInfo, MaterialComponent, NistMaterial } from '../../store/types';
import * as api from '../../api/client';
import { importNistMaterial } from '../../utils/nistImport';

export default function MaterialsPanel() {
  const materials = useAppStore((s) => s.materials);
  const elements = useAppStore((s) => s.elements);
  const selectedMaterial = useAppStore((s) => s.selectedMaterial);
  const setSelectedMaterial = useAppStore((s) => s.setSelectedMaterial);

  if (materials.length === 0 && elements.length === 0) {
    return <div style={{ color: '#666', fontSize: 12 }}>No file loaded</div>;
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
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
    </div>
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
    const materials = useAppStore.getState().materials;
    const newMat: MaterialInfo = {
      name: '',
      formula: null,
      z: null,
      density: { value: '1.0', unit: 'g/cm3' },
      density_ref: null,
      temperature: null,
      pressure: null,
      atom_value: null,
      components: [],
    };
    newMat.name = generateDefaultMaterialName(newMat, materials);
    try {
      await api.addMaterial(newMat);
      await refreshMaterials();
      onSelect(newMat.name);
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async (name: string) => {
    if (!window.confirm(`Delete material "${name}"?`)) return;
    try {
      await api.deleteMaterial(name);
      await refreshMaterials();
      if (selectedMaterial === name) onSelect(null);
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div>
      <div style={{ ...sectionHeader, marginBottom: 4 }}>
        <span>Materials ({materials.length})</span>
        <button onClick={handleAdd} style={smallBtn}>+</button>
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

function MaterialEditor({
  materialName,
  materials,
  elements,
}: {
  materialName: string;
  materials: MaterialInfo[];
  elements: ElementInfo[];
}) {
  const mat = materials.find((m) => m.name === materialName);
  const [renaming, setRenaming] = useState(false);
  const [renameTo, setRenameTo] = useState('');

  if (!mat) return null;

  const handleStartRename = () => {
    setRenameTo(mat.name);
    setRenaming(true);
  };

  const handleCommitRename = async () => {
    const newName = renameTo.trim();
    if (!newName || newName === mat.name) {
      setRenaming(false);
      return;
    }
    try {
      const updated = { ...mat, name: newName };
      await api.updateMaterial(mat.name, updated);
      await refreshMaterialsAndMeshes();
      useAppStore.getState().setSelectedMaterial(newName);
      setRenaming(false);
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div style={{ borderTop: '1px solid #0f3460', paddingTop: 6 }}>
      <div style={sectionHeader}>
        {renaming ? (
          <input
            value={renameTo}
            onChange={(e) => setRenameTo(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCommitRename();
              if (e.key === 'Escape') setRenaming(false);
            }}
            onBlur={handleCommitRename}
            style={{ ...inputStyle, flex: 1, fontSize: 11, fontWeight: 700 }}
            autoFocus
          />
        ) : (
          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            Edit: {mat.name}
          </span>
        )}
        {!renaming && (
          <button onClick={handleStartRename} style={smallBtn} title="Rename material">
            &#9998;
          </button>
        )}
      </div>
      <MaterialFields material={mat} />
      <ComponentsList material={mat} elements={elements} />
      <NistMaterialPicker material={mat} />
    </div>
  );
}

function MaterialFields({ material }: { material: MaterialInfo }) {
  const [densityVal, setDensityVal] = useState(material.density?.value || '');
  const [densityUnit, setDensityUnit] = useState(material.density?.unit || 'g/cm3');
  const [formula, setFormula] = useState(material.formula || '');
  const [z, setZ] = useState(material.z || '');
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Cancel pending save when material identity changes (e.g., after rename)
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }
    setDensityVal(material.density?.value || '');
    setDensityUnit(material.density?.unit || 'g/cm3');
    setFormula(material.formula || '');
    setZ(material.z || '');
  }, [material.name, material.density?.value, material.density?.unit, material.formula, material.z]);

  const save = useCallback(async (overrides: Partial<MaterialInfo> = {}) => {
    const updated: MaterialInfo = {
      name: material.name,
      components: material.components,
      temperature: material.temperature,
      pressure: material.pressure,
      atom_value: material.atom_value,
      density_ref: material.density_ref,
      density: { value: densityVal, unit: densityUnit },
      formula: formula || null,
      z: z || null,
      ...overrides,
    };
    try {
      await api.updateMaterial(material.name, updated);
      await refreshMaterialsAndMeshes();
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  }, [material.name, material.components, material.temperature, material.pressure, material.atom_value, material.density_ref, densityVal, densityUnit, formula, z]);

  const scheduleSave = useCallback((overrides?: Partial<MaterialInfo>) => {
    if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    saveTimeoutRef.current = setTimeout(() => {
      saveTimeoutRef.current = null;
      save(overrides);
    }, 500);
  }, [save]);

  // Flush pending save on unmount
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
        save();
      }
    };
  }, [save]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
      save();
      e.currentTarget.blur();
    }
  }, [save]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 3, marginTop: 4 }}>
      <FieldRow label="Density">
        <input
          value={densityVal}
          onChange={(e) => { setDensityVal(e.target.value); scheduleSave(); }}
          onKeyDown={handleKeyDown}
          style={{ ...inputStyle, width: 80 }}
        />
        <select
          value={densityUnit}
          onChange={(e) => { setDensityUnit(e.target.value); save({ density: { value: densityVal, unit: e.target.value } }); }}
          style={{ ...inputStyle, width: 70 }}
        >
          <option value="g/cm3">g/cm3</option>
          <option value="kg/m3">kg/m3</option>
          <option value="mg/cm3">mg/cm3</option>
        </select>
      </FieldRow>
      <FieldRow label="Formula">
        <input
          value={formula}
          onChange={(e) => { setFormula(e.target.value); scheduleSave(); }}
          onKeyDown={handleKeyDown}
          style={{ ...inputStyle, flex: 1 }}
        />
      </FieldRow>
      <FieldRow label="Z">
        <input
          value={z}
          onChange={(e) => { setZ(e.target.value); scheduleSave(); }}
          onKeyDown={handleKeyDown}
          style={{ ...inputStyle, width: 50 }}
        />
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
  elements,
}: {
  material: MaterialInfo;
  elements: ElementInfo[];
}) {
  const [addingComp, setAddingComp] = useState(false);
  const [compType, setCompType] = useState<'Fraction' | 'Composite'>('Fraction');
  const [compRef, setCompRef] = useState('');
  const [compN, setCompN] = useState('');

  const saveComponents = async (components: MaterialComponent[]) => {
    const updated: MaterialInfo = { ...material, components };
    try {
      await api.updateMaterial(material.name, updated);
      await refreshMaterialsAndMeshes();
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleAddComponent = async () => {
    if (!compRef || !compN) return;
    const newComp: MaterialComponent =
      compType === 'Fraction'
        ? { Fraction: { n: compN, ref_name: compRef } }
        : { Composite: { n: compN, ref_name: compRef } };
    await saveComponents([...material.components, newComp]);
    setAddingComp(false);
    setCompRef('');
    setCompN('');
  };

  const handleRemoveComponent = async (idx: number) => {
    const next = material.components.filter((_, i) => i !== idx);
    await saveComponents(next);
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
            <span
              onClick={() => handleRemoveComponent(i)}
              style={{ cursor: 'pointer', color: '#e94560', fontSize: 9, opacity: 0.6 }}
            >
              x
            </span>
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
            {elements.map((el) => (
              <option key={el.name} value={el.name}>{el.name}</option>
            ))}
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
    if (open) doSearch(search, category);
  }, [open, search, category, doSearch]);

  const handleApply = async (nist: NistMaterial) => {
    const oldName = material.name;
    try {
      const updated = await importNistMaterial(nist);
      await api.updateMaterial(oldName, updated);
      await refreshMaterialsAndMeshes();
      if (updated.name !== oldName) {
        useAppStore.getState().setSelectedMaterial(updated.name);
      }
      setOpen(false);
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

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
    try {
      await api.addElement({
        name: newName.trim(),
        formula: null,
        z: newZ || null,
        atom_value: newAtom || null,
      });
      await refreshMaterials();
      setAdding(false);
      setNewName('');
      setNewZ('');
      setNewAtom('');
      setElSuggestions([]);
      setShowElSuggestions(false);
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async (name: string) => {
    if (!window.confirm(`Delete element "${name}"?`)) return;
    try {
      await api.deleteElement(name);
      await refreshMaterials();
    } catch (e: unknown) {
      useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
    }
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

// ─── Helpers ────────────────────────────────────────────────────────────────

function generateDefaultMaterialName(
  mat: MaterialInfo,
  allMaterials: MaterialInfo[],
): string {
  // Determine base
  let base = 'Material';
  if (mat.formula) {
    base = mat.formula;
  } else if (mat.components.length > 0) {
    const first = mat.components[0].Fraction?.ref_name
      ?? mat.components[0].Composite?.ref_name ?? 'Material';
    base = mat.components.length > 1 ? `${first}+${mat.components.length}` : first;
  } else if (mat.z) {
    base = `Z${mat.z}`;
  }

  // Density suffix
  let suffix = '';
  if (mat.density?.value) {
    const unitTag = (!mat.density.unit || mat.density.unit === 'g/cm3')
      ? '' : `_${mat.density.unit.replace(/\//g, '')}`;
    suffix = `_d${mat.density.value}${unitTag}`;
  }

  let name = `${base}${suffix}`;

  // Uniqueness — skip self
  const others = allMaterials
    .filter((m) => m.name !== mat.name)
    .map((m) => m.name);
  if (others.includes(name)) {
    let i = 2;
    while (others.includes(`${name}_${i}`)) i++;
    name = `${name}_${i}`;
  }
  return name;
}

async function refreshMaterials() {
  const data = await api.getMaterials();
  const store = useAppStore.getState();
  store.setMaterials(data.materials);
  store.setElements(data.elements);
}

async function refreshMaterialsAndMeshes() {
  await refreshMaterials();
  try {
    const [meshData, structData] = await Promise.all([
      api.getMeshes(),
      api.getStructure(),
    ]);
    const store = useAppStore.getState();
    store.setMeshes(meshData.meshes);
    store.setSceneGraph(meshData.scene_graph);
    store.setVolumes(structData.volumes);
  } catch (e: unknown) {
    console.warn('Refresh failed:', e);
  }
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
