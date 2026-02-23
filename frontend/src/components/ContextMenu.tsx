import { useEffect, useRef } from 'react';
import ReactDOM from 'react-dom';
import { useAppStore } from '../store';

export default function ContextMenu() {
  const contextMenu = useAppStore((s) => s.contextMenu);
  const closeContextMenu = useAppStore((s) => s.closeContextMenu);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!contextMenu) return;

    const dismiss = () => closeContextMenu();
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') dismiss();
    };
    const handleMouseDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        dismiss();
      }
    };

    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleKey);
    window.addEventListener('scroll', dismiss, true);

    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleKey);
      window.removeEventListener('scroll', dismiss, true);
    };
  }, [contextMenu, closeContextMenu]);

  if (!contextMenu) return null;

  // Clamp position to stay within viewport
  const menuWidth = 160;
  const menuItemHeight = 28;
  const menuHeight = contextMenu.items.length * menuItemHeight + 8;
  const left = Math.min(contextMenu.x, window.innerWidth - menuWidth - 4);
  const top = Math.min(contextMenu.y, window.innerHeight - menuHeight - 4);

  return ReactDOM.createPortal(
    <div
      ref={menuRef}
      style={{
        position: 'fixed',
        top,
        left,
        minWidth: menuWidth,
        background: '#16213e',
        border: '1px solid #0f3460',
        borderRadius: 4,
        boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
        padding: '4px 0',
        zIndex: 10000,
        fontFamily: 'monospace',
        fontSize: 12,
      }}
    >
      {contextMenu.items.map((item, i) => (
        <div
          key={i}
          onClick={() => {
            item.action();
            closeContextMenu();
          }}
          style={{
            padding: '6px 12px',
            cursor: 'pointer',
            color: '#b0b8c0',
            userSelect: 'none',
          }}
          onMouseOver={(e) => (e.currentTarget.style.background = '#0f3460')}
          onMouseOut={(e) => (e.currentTarget.style.background = 'transparent')}
        >
          {item.label}
        </div>
      ))}
    </div>,
    document.body,
  );
}
