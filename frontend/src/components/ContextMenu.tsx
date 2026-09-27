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

    // Put the items in reach of the keyboard; the menu renders at the end of the page.
    menuRef.current?.querySelector('button')?.focus();
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
  const menuWidth = 180;
  const menuItemHeight = window.matchMedia('(pointer: coarse)').matches ? 44 : 32;
  const menuHeight = contextMenu.items.length * menuItemHeight + 8;
  const left = Math.min(contextMenu.x, window.innerWidth - menuWidth - 4);
  const top = Math.min(contextMenu.y, window.innerHeight - menuHeight - 4);

  return ReactDOM.createPortal(
    <div
      ref={menuRef}
      className="context-menu"
      style={{ top, left, minWidth: menuWidth }}
    >
      {contextMenu.items.map((item, i) => (
        <button
          key={i}
          className="plain"
          onClick={() => {
            item.action();
            closeContextMenu();
          }}
        >
          {item.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}
