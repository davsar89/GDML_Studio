import { useState, useEffect, useCallback, useRef } from 'react';
import { useAppStore } from '../../store';
import type { MaterialInfo, ElementInfo, MaterialComponent, NistMaterial } from '../../store/types';
import * as api from '../../api/client';

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
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState('');

  const handleAdd = async () => {
    if (!newName.trim()) return;
    try {
      await api.addMaterial({
        name: newName.trim(),
        formula: null,
        z: null,
        density: { value: '1.0', unit: 'g/cm3' },
        density_ref: null,
        temperature: null,
        pressure: null,
        atom_value: null,
        components: [],
      });
      await refreshMaterials();
      setAdding(false);
      setNewName('');
      onSelect(newName.trim());
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
        <button onClick={() => setAdding(!adding)} style={smallBtn}>+</button>
      </div>
      {adding && (
        <div style={{ display: 'flex', gap: 4, marginBottom: 4 }}>
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
            placeholder="Material name"
            style={inputStyle}
            autoFocus
          />
          <button onClick={handleAdd} style={smallBtn}>Add</button>
        </div>
      )}
      <div style={{ maxHeight: 160, overflow: 'auto' }}>
        {materials.map((m) => (
          <div
            key={m.name}
            onClick={() => onSelect(m.name === selectedMaterial ? null : m.name)}
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
  if (!mat) return null;

  return (
    <div style={{ borderTop: '1px solid #0f3460', paddingTop: 6 }}>
      <div style={sectionHeader}>Edit: {mat.name}</div>
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
  const dirtyRef = useRef(false);

  useEffect(() => {
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

  const handleBlur = useCallback(() => {
    if (dirtyRef.current) {
      dirtyRef.current = false;
      save();
    }
  }, [save]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      dirtyRef.current = false;
      save();
      e.currentTarget.blur();
    }
  }, [save]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 3, marginTop: 4 }}>
      <FieldRow label="Density">
        <input
          value={densityVal}
          onChange={(e) => { setDensityVal(e.target.value); dirtyRef.current = true; }}
          onBlur={handleBlur}
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
          onChange={(e) => { setFormula(e.target.value); dirtyRef.current = true; }}
          onBlur={handleBlur}
          onKeyDown={handleKeyDown}
          style={{ ...inputStyle, flex: 1 }}
        />
      </FieldRow>
      <FieldRow label="Z">
        <input
          value={z}
          onChange={(e) => { setZ(e.target.value); dirtyRef.current = true; }}
          onBlur={handleBlur}
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
      await refreshMaterials();
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
    const updated: MaterialInfo = {
      ...material,
      density: { value: String(nist.density), unit: 'g/cm3' },
    };
    try {
      await api.updateMaterial(material.name, updated);
      await refreshMaterialsAndMeshes();
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
                title={`${n.name} - ${n.density} g/cm3 (${n.state}, ${n.category})`}
              >
                <span style={{ color: '#56d6c8' }}>{n.name}</span>
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
            <div style={{ display: 'flex', gap: 3, marginBottom: 3, flexWrap: 'wrap' }}>
              <input value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="Name" style={{ ...inputStyle, width: 60 }} autoFocus />
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

async function refreshMaterials() {
  const data = await api.getMaterials();
  const store = useAppStore.getState();
  store.setMaterials(data.materials);
  store.setElements(data.elements);
}

async function refreshMaterialsAndMeshes() {
  await refreshMaterials();
  try {
    const meshData = await api.getMeshes();
    const store = useAppStore.getState();
    store.setMeshes(meshData.meshes);
    store.setSceneGraph(meshData.scene_graph);
  } catch (e: unknown) {
    console.warn('Mesh refresh failed:', e);
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
