import { useState } from 'react';
import { useAppStore } from '../../store';
import type { MaterialInfo, ElementInfo, MaterialComponent, NistMaterial } from '../../store/types';
import * as api from '../../api/client';
import { emptyMaterial, prepareNistMaterial } from '../../utils/nistImport';
import { refreshMaterials } from '../../utils/refresh';
import { useNistSearch } from '../../utils/useNistSearch';
import { applyMaterialDraft, confirmAction, resolveMaterialDraft, runDocumentAction } from '../../utils/documentActions';

export default function MaterialsPanel() {
  const materials = useAppStore((s) => s.materials);
  const busy = useAppStore((s) => s.documentBusy);
  const elements = useAppStore((s) => s.elements);
  const selectedMaterial = useAppStore((s) => s.selectedMaterial);
  const setSelectedMaterial = useAppStore((s) => s.setSelectedMaterial);
  const loaded = useAppStore((s) => s.summary !== null);

  if (!loaded) return <div className="empty-state">No file loaded</div>;
  return (
    <fieldset disabled={busy}>
      <MaterialsList materials={materials} selectedMaterial={selectedMaterial} onSelect={setSelectedMaterial} />
      <ElementsList elements={elements} />
    </fieldset>
  );
}

function MaterialsList({ materials, selectedMaterial, onSelect }: {
  materials: MaterialInfo[]; selectedMaterial: string | null; onSelect: (name: string | null) => void;
}) {
  const draft = useAppStore((s) => s.materialDraft);
  const newDraft = draft && !materials.some((m) => m.name === draft.original.name) ? draft.material : null;
  // Only a local draft: the material reaches the document on Apply.
  const handleAdd = () => runDocumentAction(async () => {
    if (!await resolveMaterialDraft()) return;
    const { materials, elements } = useAppStore.getState();
    const names = new Set([...materials, ...elements].map((item) => item.name));
    let n = 1;
    while (names.has(`Material_${n}`)) n++;
    const material = emptyMaterial(`Material_${n}`);
    useAppStore.setState({ selectedMaterial: material.name, materialDraft: { original: material, material, elements: [] } });
  });

  const handleDelete = async (name: string) => {
    await runDocumentAction(async () => {
      if (!await resolveMaterialDraft()) return;
      const target = selectedMaterial === name ? useAppStore.getState().selectedMaterial ?? name : name;
      if (!await confirmAction('Delete material', `Delete material "${target}"?`, 'Delete')) return;
      await api.deleteMaterial(target);
      await refreshMaterials();
      if (selectedMaterial === name) useAppStore.setState({ selectedMaterial: null });
    });
  };

  return (
    <section className="form-section">
      <div className="section-heading">
        <h3>Materials ({materials.length})</h3>
        <button className="quiet icon-button" aria-label="Add material" onClick={handleAdd}>+</button>
      </div>
      <div className="material-list">
        {materials.map((m) => (
          <div key={m.name} className="list-row" data-selected={m.name === selectedMaterial}
            onContextMenu={(e) => {
              e.preventDefault();
              useAppStore.getState().openContextMenu(e.clientX, e.clientY, [
                { label: 'Edit', action: () => onSelect(m.name) },
                { label: 'Delete', action: () => handleDelete(m.name) },
              ]);
            }}>
            <button className="plain row-name mono" aria-pressed={m.name === selectedMaterial}
              title={m.name} onClick={() => onSelect(m.name === selectedMaterial ? null : m.name)}>
              {m.name}
              {m.density && <small>{m.density.value} {m.density.unit || 'g/cm3'}</small>}
            </button>
            <button className="quiet icon-button danger" onClick={() => handleDelete(m.name)}
              aria-label={`Delete material ${m.name}`} title="Delete material">×</button>
          </div>
        ))}
        {newDraft && <div className="draft-row mono">{newDraft.name} <span>unapplied</span></div>}
      </div>
    </section>
  );
}

// Mounted in the inspector, keyed by material so the catalog and pickers start fresh.
export function MaterialEditor({ materialName, materials, elements }: {
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
    <div>
      <h3 className="editor-title">Edit: {materialName}</h3>
      <section className="form-section">
        <label className="field">Name
          <input aria-label="Material name" value={material.name} onChange={(e) => update({ name: e.target.value })} />
        </label>
        <MaterialFields material={material} onChange={update} />
      </section>
      <ComponentsList material={material} materials={materials} elements={availableElements} onChange={update} />
      <NistMaterialPicker material={material} />
      {draftError && <div role="alert" className="validation-error">{draftError}</div>}
      <div className="actions editor-actions">
        <button className="primary" disabled={!draft} onClick={() => runDocumentAction(applyMaterialDraft)}>Apply</button>
        <button disabled={!draft} onClick={() => useAppStore.getState().discardMaterialDraft()}>Cancel</button>
      </div>
    </div>
  );
}

function MaterialFields({ material, onChange }: {
  material: MaterialInfo; onChange: (changes: Partial<MaterialInfo>) => void;
}) {
  return (
    <div className="form-grid">
      <label className="field">Density
        <input aria-label="Density" value={material.density?.value ?? ''}
          onChange={(e) => onChange({ density: { value: e.target.value, unit: material.density?.unit ?? 'g/cm3' }, density_ref: null })} />
      </label>
      <label className="field">Unit
        <select aria-label="Density unit" value={material.density?.unit ?? 'g/cm3'}
          onChange={(e) => onChange({ density: { value: material.density?.value ?? '', unit: e.target.value }, density_ref: null })}>
          <option value="g/cm3">g/cm3</option><option value="kg/m3">kg/m3</option><option value="mg/cm3">mg/cm3</option>
        </select>
      </label>
      <label className="field full-width">Formula
        <input aria-label="Formula" value={material.formula ?? ''} onChange={(e) => onChange({ formula: e.target.value || null })} />
      </label>
      <label className="field">Atomic number (Z)
        <input aria-label="Atomic number" value={material.z ?? ''} onChange={(e) => onChange({ z: e.target.value || null })} />
      </label>
      <label className="field">Atomic mass (A)
        <input aria-label="Atomic mass" value={material.atom_value ?? ''} onChange={(e) => onChange({ atom_value: e.target.value || null })} />
        <span className="field-hint">{material.atom_unit ?? 'g/mole'}</span>
      </label>
    </div>
  );
}

function ComponentsList({ material, materials, elements, onChange }: {
  material: MaterialInfo; materials: MaterialInfo[]; elements: ElementInfo[]; onChange: (changes: Partial<MaterialInfo>) => void;
}) {
  const [addingComp, setAddingComp] = useState(false);
  const [compType, setCompType] = useState<'Fraction' | 'Composite'>('Fraction');
  const [compRef, setCompRef] = useState('');
  const [compN, setCompN] = useState('');

  const handleAddComponent = () => {
    if (!compRef || !compN) return;
    const newComp: MaterialComponent = compType === 'Fraction'
      ? { Fraction: { n: compN, ref_name: compRef } } : { Composite: { n: compN, ref_name: compRef } };
    onChange({ components: [...material.components, newComp] });
    setAddingComp(false);
    setCompRef('');
    setCompN('');
  };

  return (
    <section className="form-section">
      <div className="section-heading">
        <h3>Components ({material.components.length})</h3>
        <button className="quiet icon-button" aria-label="Add component" aria-expanded={addingComp} onClick={() => setAddingComp(!addingComp)}>+</button>
      </div>
      {material.components.map((c, i) => {
        const { n, ref_name: ref } = (c.Fraction ?? c.Composite)!;
        return (
          <div key={i} className="component-row">
            <span className="mono">{ref}</span>
            <span className="muted">{c.Fraction ? `mass ${n}` : `${n} atom${n === '1' ? '' : 's'}`}</span>
            <button className="quiet icon-button danger" aria-label={`Remove component ${ref}`}
              onClick={() => onChange({ components: material.components.filter((_, index) => index !== i) })}>×</button>
          </div>
        );
      })}
      {addingComp && (
        <div className="form-grid">
          <label className="field">Type
            <select value={compType} onChange={(e) => setCompType(e.target.value as 'Fraction' | 'Composite')}>
              <option value="Fraction">Fraction</option><option value="Composite">Composite</option>
            </select>
          </label>
          <label className="field">Weight / count
            <input value={compN} onChange={(e) => setCompN(e.target.value)} placeholder="n" />
          </label>
          <label className="field full-width">Reference
            <select value={compRef} onChange={(e) => setCompRef(e.target.value)}>
              <option value="">Select a reference</option>
              <optgroup label="Elements">{elements.map((el) => <option key={`element-${el.name}`} value={el.name}>{el.name}</option>)}</optgroup>
              <optgroup label="Materials">{materials.filter((candidate) => candidate.name !== material.name)
                .map((candidate) => <option key={`material-${candidate.name}`} value={candidate.name}>{candidate.name}</option>)}</optgroup>
            </select>
          </label>
          <button onClick={handleAddComponent}>Add</button>
        </div>
      )}
    </section>
  );
}

function NistMaterialPicker({ material }: { material: MaterialInfo }) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('');
  const { results, loading } = useNistSearch(open ? search : null, category);

  const handleApply = (nist: NistMaterial) => runDocumentAction(async () => {
    const prepared = await prepareNistMaterial(nist);
    // The catalog supplies composition and density, not these GDML properties.
    const { temperature, temperature_ref, pressure, pressure_ref, mee, mee_ref, rl, al, properties } = material;
    useAppStore.getState().updateMaterialDraft({
      ...prepared.material, temperature, temperature_ref, pressure, pressure_ref, mee, mee_ref, rl, al, properties,
    }, prepared.elements);
    setOpen(false);
  });

  return (
    <section className="form-section">
      <button onClick={() => setOpen(!open)} aria-expanded={open}>{open ? 'Close NIST Lookup' : 'NIST Material Lookup'}</button>
      {open && <>
        <div className="form-grid">
          <label className="field full-width">Search catalog
            <input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search..." autoFocus />
          </label>
          <label className="field full-width">Category
            <select value={category} onChange={(e) => setCategory(e.target.value)}>
              <option value="">All</option><option value="Elemental">Elemental</option><option value="Compound">Compound</option>
              <option value="HEP">HEP</option><option value="Space">Space</option><option value="Biochemical">Biochemical</option>
            </select>
          </label>
        </div>
        <div className="catalog-results">
          {loading && <div className="muted" role="status">Loading…</div>}
          {results.map((n) => (
            <button key={n.name} className="plain catalog-option" onClick={() => handleApply(n)}
              title={`${n.name}${n.formula ? ` (${n.formula})` : ''}: ${n.density} g/cm3 (${n.state}, ${n.category})`}>
              <span className="mono">{n.name}</span>
              <span className="muted">{n.formula && `${n.formula} · `}{n.density} g/cm3 · {n.state}</span>
            </button>
          ))}
          {!loading && results.length === 0 && <div className="muted">No results</div>}
        </div>
      </>}
    </section>
  );
}

function ElementsList({ elements }: { elements: ElementInfo[] }) {
  const [expanded, setExpanded] = useState(false);
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState('');
  const [newZ, setNewZ] = useState('');
  const [newAtom, setNewAtom] = useState('');
  const [showElSuggestions, setShowElSuggestions] = useState(false);
  const elSuggestions = useNistSearch(newName || null, 'Elemental').results.slice(0, 10);

  const handleSelectSuggestion = (nist: NistMaterial) => {
    setNewName(nist.formula ?? nist.name);
    if (nist.z != null) setNewZ(String(nist.z));
    if (nist.atom_value != null) setNewAtom(String(nist.atom_value));
    setShowElSuggestions(false);
  };

  const handleAdd = async () => {
    await runDocumentAction(async () => {
      await api.addElement({ name: newName.trim(), formula: null, z: newZ || null, atom_value: newAtom || null,
        atom_unit: null, atom_type: null, fractions: [] });
      await refreshMaterials();
      setAdding(false);
      setNewName('');
      setNewZ('');
      setNewAtom('');
      setShowElSuggestions(false);
    });
  };

  const handleDelete = async (name: string) => {
    await runDocumentAction(async () => {
      if (!await confirmAction('Delete element', `Delete element "${name}"?`, 'Delete')) return;
      await api.deleteElement(name);
      await refreshMaterials();
    });
  };

  return (
    <section className="form-section">
      <div className="section-heading">
        <button className="plain" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>
          <span aria-hidden>{expanded ? '▾' : '▸'}</span> Elements ({elements.length})
        </button>
        <button className="quiet icon-button" aria-label="Add element" onClick={() => { setAdding(!adding); setExpanded(true); }}>+</button>
      </div>
      {expanded && <>
        {adding && <div className="form-grid">
          <div className="popover-anchor full-width" onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) setShowElSuggestions(false); }}>
            <label className="field">Name
              <input value={newName} onChange={(e) => { setNewName(e.target.value); setShowElSuggestions(true); }}
                onFocus={() => setShowElSuggestions(true)} placeholder="Name" autoFocus />
            </label>
            {showElSuggestions && elSuggestions.length > 0 && (
              <div className="popover" onMouseDown={(e) => e.preventDefault()}>
                {elSuggestions.map((s) => <button key={s.name} className="plain catalog-option" onClick={() => handleSelectSuggestion(s)}>
                  <span className="mono">{s.name}</span>
                  <span className="muted">{s.z != null && `Z=${s.z} `}{s.atom_value != null && `A=${s.atom_value}`}</span>
                </button>)}
              </div>
            )}
          </div>
          <label className="field">Atomic number
            <input value={newZ} onChange={(e) => setNewZ(e.target.value)} placeholder="Z" />
          </label>
          <label className="field">Atomic mass
            <input value={newAtom} onChange={(e) => setNewAtom(e.target.value)} placeholder="Atom" />
          </label>
          <button onClick={handleAdd} disabled={!newName.trim() || !newZ.trim() || !newAtom.trim()}>Add</button>
        </div>}
        {elements.map((el) => <div key={el.name} className="list-row">
          <span className="row-name" title={el.name}>
            <span className="mono">{el.name}</span>{' '}
            <span className="field-hint">{el.z && `Z=${el.z} `}{el.atom_value && `A=${el.atom_value}`}</span>
          </span>
          <button className="quiet icon-button danger" onClick={() => handleDelete(el.name)} aria-label={`Delete element ${el.name}`} title="Delete element">×</button>
        </div>)}
      </>}
    </section>
  );
}
