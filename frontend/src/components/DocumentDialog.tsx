import { useEffect, useId, useRef } from 'react';
import { useAppStore } from '../store';

export default function DocumentDialog() {
  const decision = useAppStore((s) => s.decision);
  const ref = useRef<HTMLDialogElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const titleId = useId();
  const messageId = useId();
  useEffect(() => {
    if (!decision || !ref.current) return;
    const dialog = ref.current;
    dialog.showModal();
    // Set per decision, so a previous dialog's typing never carries over.
    const input = inputRef.current;
    if (input) { input.value = decision.input ?? ''; input.select(); }
    else dialog.querySelector<HTMLButtonElement>('[data-cancel]')?.focus();
    // runDocumentAction returns focus to the control that opened the dialog.
    return () => dialog.close();
  }, [decision]);

  if (!decision) return null;
  const submit = () => decision.resolve('primary', inputRef.current?.value);
  return (
    <dialog ref={ref} aria-labelledby={titleId} aria-describedby={messageId}
      onCancel={(e) => { e.preventDefault(); decision.resolve('cancel'); }}
      className="document-dialog">
      <h2 id={titleId}>{decision.title}</h2>
      <p id={messageId}>{decision.message}</p>
      {decision.input !== undefined && (
        <input ref={inputRef} aria-labelledby={messageId} onKeyDown={(e) => { if (e.key === 'Enter') submit(); }} />
      )}
      <div className="actions">
        <button className={decision.primaryLabel === 'Delete' ? 'danger' : 'primary'} onClick={submit}>{decision.primaryLabel}</button>
        {decision.canDiscard && <button onClick={() => decision.resolve('discard')}>Discard</button>}
        <button data-cancel onClick={() => decision.resolve('cancel')}>Cancel</button>
      </div>
    </dialog>
  );
}
