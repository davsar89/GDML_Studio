import * as api from '../api/client';
import { useAppStore } from '../store';
import { refreshMaterialsAndScene } from './refresh';

declare global {
  interface Window {
    /** Set by the desktop app's preload script (desktop/preload.js); absent in a browser. */
    desktop?: {
      /** Native Save dialog, then write. The saved file's name, or null when cancelled. */
      saveFile(content: string, filename: string): Promise<string | null>;
    };
  }
}

export type DialogChoice = 'primary' | 'discard' | 'cancel';
export interface DocumentDecision {
  title: string;
  message: string;
  primaryLabel: string;
  canDiscard: boolean;
  /** Initial text of a one-line input; its final text is passed to `resolve`. */
  input?: string;
  resolve: (choice: DialogChoice, text?: string) => void;
}

/** Show the in-app dialog. Native prompt/confirm would block the whole tab. */
function ask(request: Omit<DocumentDecision, 'resolve'>): Promise<{ choice: DialogChoice; text: string }> {
  return new Promise((resolve) => {
    useAppStore.setState({ decision: {
      ...request,
      resolve: (choice, text = '') => { useAppStore.setState({ decision: null }); resolve({ choice, text }); },
    } });
  });
}

export async function askDocumentDecision(
  title: string, message: string, primaryLabel: 'Apply' | 'Save',
): Promise<DialogChoice> {
  return (await ask({ title, message, primaryLabel, canDiscard: true })).choice;
}

export async function confirmAction(title: string, message: string, primaryLabel: string): Promise<boolean> {
  return (await ask({ title, message, primaryLabel, canDiscard: false })).choice === 'primary';
}

/** The file name to save as, with a .gdml extension, or null when cancelled. */
export async function askFilename(initial: string): Promise<string | null> {
  const { choice, text } = await ask({ title: 'Save As', message: 'File name', primaryLabel: 'Save', canDiscard: false, input: initial });
  const name = text.trim();
  if (choice !== 'primary' || !name) return null;
  return /\.gdml$/i.test(name) ? name : `${name}.gdml`;
}

/** One document action at a time, including the dialogs that precede it. */
export async function runDocumentAction(action: () => Promise<void>): Promise<void> {
  if (useAppStore.getState().documentBusy) return;
  const initiator = document.activeElement;
  useAppStore.setState({ documentBusy: true, error: null });
  try {
    await action();
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    // A failed Apply already shows its error next to the draft.
    if (useAppStore.getState().draftError !== message) useAppStore.getState().setError(message);
  } finally {
    useAppStore.setState({ documentBusy: false });
    // The lock may have disabled the control that started the action, and its dialogs took focus.
    // Return focus once React has re-enabled that control, unless the user moved it elsewhere.
    requestAnimationFrame(() => {
      if (initiator instanceof HTMLElement && document.activeElement === document.body) initiator.focus();
    });
  }
}

/** Caller owns the document action; a failed Apply leaves the complete draft. */
export async function applyMaterialDraft(): Promise<void> {
  const { materialDraft: draft, materials } = useAppStore.getState();
  if (!draft) return;
  const isNew = !materials.some((m) => m.name === draft.original.name);
  useAppStore.setState({ draftError: null });
  try {
    await (isNew
      ? api.addMaterial(draft.material, draft.elements)
      : api.updateMaterial(draft.original.name, draft.material, draft.elements));
  } catch (e: unknown) {
    useAppStore.setState({ draftError: e instanceof Error ? e.message : String(e), activeTreeTab: 'materials' });
    throw e;
  }
  // Commit the acknowledged value locally before refreshing. A failed refresh
  // must not turn a successfully applied rename into a stale second request.
  useAppStore.setState((state) => ({
    materials: isNew ? [...state.materials, draft.material]
      : state.materials.map((m) => m.name === draft.original.name ? draft.material : m),
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

/**
 * Export only after the caller has resolved drafts and acquired the action.
 * False when the desktop Save dialog was cancelled: nothing was written, so the
 * document stays unsaved. A browser download cannot report that and counts as saved.
 */
export async function exportCurrentDocument(filename?: string): Promise<boolean> {
  const { editRevision, summary } = useAppStore.getState();
  const result = await api.exportGdml();
  let saved: string | null = filename ?? summary?.filename ?? result.filename;
  if (window.desktop) saved = await window.desktop.saveFile(result.gdml, saved);
  else downloadFile(result.gdml, saved);
  if (saved === null) return false;
  useAppStore.getState().markSaved(editRevision);
  // The next Save keeps the name just saved under.
  useAppStore.setState((s) => ({ summary: s.summary && { ...s.summary, filename: saved } }));
  return true;
}

/** Runs after a replacement file was actually selected, before uploading it. */
export async function resolveDocumentReplacement(): Promise<boolean> {
  if (!await resolveMaterialDraft()) return false;
  if (!useAppStore.getState().dirty) return true;
  const choice = await askDocumentDecision(
    'Unsaved document changes', 'Save the current document before opening another file?', 'Save',
  );
  if (choice === 'cancel') return false;
  // A cancelled Save dialog keeps the current document open, as Cancel does.
  if (choice === 'primary') return exportCurrentDocument();
  return true;
}
