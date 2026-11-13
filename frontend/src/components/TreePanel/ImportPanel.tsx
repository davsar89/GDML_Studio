import { useState } from 'react';
import { useAppStore } from '../../store';
import type { ImportDefinition, ImportOptions, ImportOverride, ImportReport } from '../../store/types';
import { reimport } from '../../utils/cadImport';
import type { MaterialChoice } from '../../utils/cadImport';
import { runDocumentAction, resolveMaterialDraft } from '../../utils/documentActions';
import SearchableCombobox from '../SearchableCombobox';

/** STL files carry no unit; the ones offered, as the length of one unit in mm. */
const STL_UNITS: [string, number][] = [['mm', 1], ['cm', 10], ['m', 1000], ['in', 25.4]];

/** The mesh volume errors offered for STEP parts. */
const QUALITIES: [string, number][] = [['Coarse: 1 %', 1e-2], ['Standard: 0.1 %', 1e-3], ['Fine: 0.01 %', 1e-4]];

type Representation = ImportOverride['representation'];

/** The options being edited; Re-import applies them. Chords are typed per part, in mm. */
interface Draft extends Omit<ImportOptions, 'part_material' | 'overrides'> {
  material: MaterialChoice | null;
  representations: Record<string, Representation>;
  chords: Record<string, string>;
}

const draftOf = ({ options }: ImportReport): Draft => ({
  stl_unit_mm: options.stl_unit_mm,
  volume_target: options.volume_target,
  hierarchy: options.hierarchy,
  material: options.part_material && { name: options.part_material.material.name },
  representations: Object.fromEntries(Object.entries(options.overrides).map(([id, o]) => [id, o.representation])),
  chords: Object.fromEntries(Object.entries(options.overrides).flatMap(([id, o]) => o.chord_mm ? [[id, String(o.chord_mm)]] : [])),
});

/** The per-part choices of `draft`, as the backend takes them. */
function overridesOf(draft: Draft): Record<string, ImportOverride> {
  const out: Record<string, ImportOverride> = {};
  for (const [id, representation] of Object.entries(draft.representations)) {
    if (representation !== 'auto') out[id] = { representation };
  }
  for (const [id, chord] of Object.entries(draft.chords)) {
    if (chord.trim()) out[id] = { representation: out[id]?.representation ?? 'auto', chord_mm: Number(chord) };
  }
  return out;
}

const draftKey = (d: Draft) => JSON.stringify([d.stl_unit_mm, d.volume_target, d.hierarchy, d.material?.name ?? null,
  Object.entries(overridesOf(d)).sort()]);

const count = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString('en-US')} ${n === 1 ? one : many}`;
const quantity = (x: number) => x.toLocaleString('en-US', { maximumSignificantDigits: 8 });
const percent = (x: number) => `${(100 * x).toLocaleString('en-US', { maximumSignificantDigits: 2, signDisplay: 'exceptZero' })} %`;
const size = (bytes: number) => bytes < 1e6
  ? `${(bytes / 1e3).toLocaleString('en-US', { maximumFractionDigits: 1 })} kB`
  : `${(bytes / 1e6).toLocaleString('en-US', { maximumFractionDigits: 1 })} MB`;

/** What repairing the part's mesh changed, in words. */
function repairs({ repairs: r, reversed_facets }: ImportDefinition): string[] {
  return [
    r.welded_vertices > 0 && `${count(r.welded_vertices, 'vertex', 'vertices')} welded (up to ${quantity(r.weld_max_mm)} mm)`,
    r.dropped_facets > 0 && `${count(r.dropped_facets, 'facet')} dropped`,
    r.collapsed_edges > 0 && `${count(r.collapsed_edges, 'short edge')} collapsed`,
    r.flipped_edges > 0 && `${count(r.flipped_edges, 'edge')} flipped`,
    reversed_facets > 0 && `${count(reversed_facets, 'facet')} rewound`,
  ].filter((s) => s !== false);
}

/** The report of the open document's CAD import, with its options for a re-import. */
export default function ImportPanel() {
  const report = useAppStore((s) => s.importReport);
  return report && <ImportForm report={report} />;
}

function ImportForm({ report }: { report: ImportReport }) {
  const volumes = useAppStore((s) => s.volumes);
  const materials = useAppStore((s) => s.materials);
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const busy = useAppStore((s) => s.documentBusy);
  const [draft, setDraft] = useState(() => draftOf(report));
  // Each load brings a new report; a draft for the previous one is void.
  const [drafted, setDrafted] = useState(report);
  if (drafted !== report) { setDrafted(report); setDraft(draftOf(report)); }

  const { setSelectedVolume } = useAppStore.getState();
  const materialOf = new Map(volumes.map((v) => [v.name, v.material_ref]));
  const unassigned = volumes.filter((v) => v.material_ref === report.placeholder_material);
  const labelOf = new Map(report.definitions.map((d) => [d.id, d.label]));
  const crossed = new Map<string, string[]>();
  for (const [a, b] of report.crossings) {
    crossed.set(a, [...crossed.get(a) ?? [], b === a ? 'another instance of it' : labelOf.get(b)!]);
    if (b !== a) crossed.set(b, [...crossed.get(b) ?? [], labelOf.get(a)!]);
  }
  const interferences = [...report.interferences].sort((a, b) => b.volume_mm3 - a.volume_mm3);
  const imported = report.definitions.filter((d) => !d.excluded).length;
  const exact = report.definitions.filter((d) => !d.excluded && d.representation === 'exact').length;
  const changed = draftKey(draft) !== draftKey(draftOf(report));
  const materialChanged = (draft.material?.name ?? null) !== (report.options.part_material?.material.name ?? null);

  const choose = (id: string, representation: Representation) =>
    setDraft({ ...draft, representations: { ...draft.representations, [id]: representation } });
  const apply = () => runDocumentAction(async () => {
    if (!await resolveMaterialDraft()) return;
    const { stl_unit_mm, volume_target, hierarchy } = draft;
    await reimport({ stl_unit_mm, volume_target, hierarchy, overrides: overridesOf(draft) }, draft.material);
  });

  return (
    <fieldset disabled={busy} className="import-panel">
      <section className="form-section">
        <dl className="properties">
          <dt>File</dt><dd>{report.filename} ({report.format.toUpperCase()}, {size(report.bytes)})</dd>
          <dt>Parts</dt>
          <dd>
            {imported} of {report.definitions.length} imported{exact > 0 && ` (${exact} exact, ${imported - exact} tessellated)`}
            {report.rejected.length > 0 && `, ${report.rejected.length} rejected`}
          </dd>
          <dt>Facets</dt><dd>{report.facets.toLocaleString('en-US')}</dd>
          <dt>GDML size</dt><dd>about {size(report.gdml_bytes_estimate)}</dd>
        </dl>
        {unassigned.length > 0 && (
          <button className="attention" onClick={() => setSelectedVolume(unassigned[0].name)} title="Select the first part without a material">
            {count(unassigned.length, 'part')} without a material ({report.placeholder_material})
          </button>
        )}
      </section>

      <section className="form-section">
        <h3>Options</h3>
        {report.format === 'stl' ? (
          <label className="field">STL unit
            <select value={draft.stl_unit_mm} onChange={(e) => setDraft({ ...draft, stl_unit_mm: Number(e.target.value) })}>
              {STL_UNITS.map(([unit, mm]) => <option key={unit} value={mm}>{unit}</option>)}
            </select>
          </label>
        ) : <>
          <label className="field">Mesh volume error
            <select value={draft.volume_target} onChange={(e) => setDraft({ ...draft, volume_target: Number(e.target.value) })}>
              {QUALITIES.map(([name, target]) => <option key={target} value={target}>{name}</option>)}
            </select>
          </label>
          <label className="field">Assemblies
            <select value={draft.hierarchy} onChange={(e) => setDraft({ ...draft, hierarchy: e.target.value as Draft['hierarchy'] })}>
              <option value="keep">Keep as assembly volumes</option>
              <option value="flatten">Flatten into the world</option>
            </select>
          </label>
        </>}
        <SearchableCombobox label="Part material" value={draft.material?.name ?? report.placeholder_material} materials={materials}
          onChange={(name, nist) => setDraft({ ...draft, material: name === report.placeholder_material ? null : { name, nist } })} />
        {draft.material && <button className="quiet" onClick={() => setDraft({ ...draft, material: null })}>Clear part material</button>}
        {materialChanged && <p className="field-hint">Re-importing with a new part material replaces the material of every part.</p>}
      </section>

      <section className="form-section">
        <h3>Parts</h3>
        {report.format === 'step' && (
          <p className="field-hint">Beside each part, the chordal deviation of its mesh in mm: type one to mesh that part finer or coarser.</p>
        )}
        <ul className="import-parts">
          {report.definitions.map((d) => {
            const notes = repairs(d);
            return (
              <li key={d.id} className="list-row" data-selected={d.volume !== null && d.volume === selectedVolume}>
                <select value={draft.representations[d.id] ?? 'auto'} aria-label={`Representation of ${d.label}`}
                  onChange={(e) => choose(d.id, e.target.value as Representation)}>
                  <option value="auto">{d.analytic ? 'Exact' : 'Mesh'}</option>
                  {d.analytic && <option value="tessellated">Mesh</option>}
                  <option value="exclude">Exclude</option>
                </select>
                <button className="plain row-name" disabled={!d.volume} onClick={() => d.volume && setSelectedVolume(d.volume)}
                  title={d.volume ?? 'Not in the document'}>
                  <span className="mono">{d.label}</span>
                  <small>
                    {d.volume ? materialOf.get(d.volume) : 'excluded'}{d.instances > 1 && ` · ${count(d.instances, 'instance')}`} · {count(d.facets, 'facet')} · {quantity(d.volume_mm3)} mm³
                  </small>
                  {d.exact && (
                    <small title={`Chordal deviation ${quantity(d.exact.chord_mm)} mm; B-rep tolerance ${quantity(d.exact.tolerance_mm)} mm`}
                      className={Math.abs(d.volume_mm3 / d.exact.volume_mm3 - 1) > report.options.volume_target ? 'attention' : undefined}>
                      CAD volume {quantity(d.exact.volume_mm3)} mm³, mesh {percent(d.volume_mm3 / d.exact.volume_mm3 - 1)}
                    </small>
                  )}
                  {d.analytic && (
                    <small title={`Proven equal to the CAD solid within ${quantity(d.epsilon_mm!)} mm`}>GDML {d.analytic}</small>
                  )}
                  {report.format === 'step' && d.analytic_reason && <small>No exact GDML solid: {d.analytic_reason}</small>}
                  {notes.length > 0 && <small>Repaired: {notes.join(', ')}</small>}
                  {d.issues.length > 0 && <small className="attention">{d.issues.join('; ')}</small>}
                  {crossed.has(d.id) && <small className="attention">Mesh crosses {crossed.get(d.id)!.join(', ')}</small>}
                </button>
                {d.exact && (
                  <input type="number" className="chord" min={0} step="any" value={draft.chords[d.id] ?? ''}
                    placeholder={quantity(d.exact.chord_mm)} aria-label={`Chord of ${d.label} in mm`}
                    onChange={(e) => setDraft({ ...draft, chords: { ...draft.chords, [d.id]: e.target.value } })} />
                )}
              </li>
            );
          })}
        </ul>
      </section>

      {report.rejected.length > 0 && (
        <section className="form-section">
          <h3>Not imported</h3>
          <ul className="import-rejected">
            {report.rejected.map((r, i) => <li key={i}><span className="mono">{r.label}</span>: {r.reason}</li>)}
          </ul>
        </section>
      )}

      {(interferences.length > 0 || report.unchecked > 0) && (
        <section className="form-section">
          <h3>Overlaps in the CAD file</h3>
          {interferences.length > 0 && (
            <details>
              <summary>{count(interferences.length, 'pair')} of solids overlap, by up to {quantity(interferences[0].volume_mm3)} mm³</summary>
              <ul className="import-rejected">
                {interferences.map((i, k) => (
                  <li key={k}><span className="mono">{i.paths[0]}</span> and <span className="mono">{i.paths[1]}</span>: {quantity(i.volume_mm3)} mm³</li>
                ))}
              </ul>
            </details>
          )}
          {report.unchecked > 0 && (
            <p className="attention">{count(report.unchecked, 'pair')} of solids close enough to overlap were not checked: the check ran out of time.</p>
          )}
        </section>
      )}

      {report.mixed_contacts.length > 0 && (
        <section className="form-section">
          <h3>Exact parts touching meshes</h3>
          <p className="field-hint">
            A mesh can cut into an exact solid it touches by up to its chordal deviation, which Geant4 reports as an overlap.
          </p>
          <ul className="import-rejected">
            {report.mixed_contacts.map(([a, b], k) => (
              <li key={k}><span className="mono">{labelOf.get(a)}</span> and <span className="mono">{labelOf.get(b)}</span></li>
            ))}
          </ul>
        </section>
      )}

      {changed && (
        <div className="import-actions actions">
          <button className="primary" onClick={apply}>Re-import</button>
          <button className="quiet" onClick={() => setDraft(draftOf(report))}>Reset</button>
        </div>
      )}
    </fieldset>
  );
}
