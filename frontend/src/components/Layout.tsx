import TreePanel from './TreePanel/TreePanel';
import Viewport from './Viewport/Viewport';
import Inspector from './Inspector';
import { ViewportTools } from './Toolbar';
import { useAppStore } from '../store';

export default function Layout() {
  const summary = useAppStore((s) => s.summary);
  const measureMode = useAppStore((s) => s.measureMode);
  return (
    <main className="workspace">
      <TreePanel />
      <section className="viewport-area" aria-label="Geometry viewport">
        <ViewportTools />
        <Viewport />
        <footer className="viewport-status">
          <span>{summary ? `${summary.solids_count} solids · ${summary.volumes_count} volumes · ${summary.meshes_count} meshes` : 'No geometry loaded'}</span>
          <span>{measureMode ? 'Click two points · Right-drag to orbit · Esc to cancel' : 'Drag to orbit · Scroll to zoom · Right-drag to pan'}</span>
        </footer>
      </section>
      <Inspector />
    </main>
  );
}
