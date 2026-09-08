import * as api from '../api/client';
import { useAppStore } from '../store';
import { refreshMaterialsAndScene } from './refresh';

export type DialogChoice = 'primary' | 'discard' | 'cancel';
export interface DocumentDecision {
  title: string;
  message: string;
  primaryLabel: 'Apply' | 'Save';
  resolve: (choice: DialogChoice) => void;
}

export function askDocumentDecision(
  title: string, message: string, primaryLabel: 'Apply' | 'Save',
): Promise<DialogChoice> {
  return new Promise((resolve) => {
    useAppStore.setState({ decision: {
      title, message, primaryLabel,
      resolve: (choice) => { useAppStore.setState({ decision: null }); resolve(choice); },
    } });
  });
}

/** One document action at a time, including the dialogs that precede it. */
export async function runDocumentAction(action: () => Promise<void>): Promise<void> {
  const store = useAppStore.getState();
  if (store.documentBusy || store.loading) return;
  useAppStore.setState({ documentBusy: true, error: null });
  try {
    await action();
  } catch (e: unknown) {
    useAppStore.getState().setError(e instanceof Error ? e.message : String(e));
  } finally {
    useAppStore.setState({ documentBusy: false });
  }
}

/** Caller owns the document action; a failed Apply leaves the complete draft. */
export async function applyMaterialDraft(): Promise<void> {
  const draft = useAppStore.getState().materialDraft;
  if (!draft) return;
  useAppStore.setState({ draftError: null });
  try {
    await api.updateMaterial(draft.original.name, draft.material, draft.elements);
  } catch (e: unknown) {
    useAppStore.setState({ draftError: e instanceof Error ? e.message : String(e), activeTreeTab: 'materials' });
    throw e;
  }
  // Commit the acknowledged value locally before refreshing. A failed refresh
  // must not turn a successfully applied rename into a stale second request.
  useAppStore.setState((state) => ({
    materials: state.materials.map((m) => m.name === draft.original.name ? draft.material : m),
    elements: [...state.elements, ...draft.elements.filter((el) => !state.elements.some((existing) => existing.name === el.name))],
    selectedMaterial: draft.material.name,
    materialDraft: null,
    draftError: null,
  }));
  await refreshMaterialsAndScene();
}

export async function resolveMaterialDraft(): Promise<boolean> {
  const draft = useAppStore.getState().materialDraft;
  if (!draft) return true;
  const choice = await askDocumentDecision(
    'Unapplied material changes', `Apply changes to "${draft.original.name}" before continuing?`, 'Apply',
  );
  if (choice === 'cancel') return false;
  if (choice === 'primary') await applyMaterialDraft();
  else useAppStore.getState().discardMaterialDraft();
  return true;
}

export function downloadFile(content: string, filename: string): void {
  const url = URL.createObjectURL(new Blob([content], { type: 'application/xml' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  setTimeout(() => { anchor.remove(); URL.revokeObjectURL(url); }, 0);
}

/** Export only after the caller has resolved drafts and acquired the action. */
export async function exportCurrentDocument(filename?: string): Promise<void> {
  const { editRevision, summary } = useAppStore.getState();
  const result = await api.exportGdml();
  downloadFile(result.gdml, filename ?? summary?.filename ?? result.filename);
  useAppStore.getState().markSaved(editRevision);
}

/** Runs after a replacement file was actually selected, before uploading it. */
export async function resolveDocumentReplacement(): Promise<boolean> {
  if (!await resolveMaterialDraft()) return false;
  if (!useAppStore.getState().dirty) return true;
  const choice = await askDocumentDecision(
    'Unsaved document changes', 'Save the current document before opening another file?', 'Save',
  );
  if (choice === 'cancel') return false;
  if (choice === 'primary') await exportCurrentDocument();
  return true;
}
