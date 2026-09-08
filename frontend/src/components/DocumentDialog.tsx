import { useEffect, useId, useRef } from 'react';
import { useAppStore } from '../store';

export default function DocumentDialog() {
  const decision = useAppStore((s) => s.decision);
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const messageId = useId();
  useEffect(() => {
    if (!decision || !ref.current) return;
    const previous = document.activeElement;
    const dialog = ref.current;
    dialog.showModal();
    dialog.querySelector<HTMLButtonElement>('[data-cancel]')?.focus();
    return () => {
      dialog.close();
      // The initiating control is disabled until the document action releases
      // its lock. Restore focus after React has committed that state change.
      requestAnimationFrame(() => {
        if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
      });
    };
  }, [decision]);

  if (!decision) return null;
  return (
    <dialog ref={ref} aria-labelledby={titleId} aria-describedby={messageId}
      onCancel={(e) => { e.preventDefault(); decision.resolve('cancel'); }}
      style={{ background: '#16213e', color: '#e0e0e0', border: '1px solid #8899aa', borderRadius: 6, padding: 24, margin: 'auto', width: 'calc(100vw - 48px)', maxWidth: 420 }}>
      <h2 id={titleId} style={{ fontSize: 18, margin: '0 0 12px' }}>{decision.title}</h2>
      <p id={messageId} style={{ marginBottom: 20, lineHeight: 1.5 }}>{decision.message}</p>
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 12 }}>
        <button style={{ ...buttonStyle, background: '#c52f49' }} onClick={() => decision.resolve('primary')}>{decision.primaryLabel}</button>
        <button style={buttonStyle} onClick={() => decision.resolve('discard')}>Discard</button>
        <button style={buttonStyle} data-cancel onClick={() => decision.resolve('cancel')}>Cancel</button>
      </div>
    </dialog>
  );
}

const buttonStyle = { background: '#0f3460', color: '#fff', border: '1px solid #8899aa', borderRadius: 4, padding: '6px 12px', fontSize: 14, cursor: 'pointer' };
