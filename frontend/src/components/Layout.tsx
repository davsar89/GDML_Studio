import TreePanel from './TreePanel/TreePanel';
import Viewport from './Viewport/Viewport';

export default function Layout() {
  return (
    <div
      style={{
        flex: 1,
        display: 'grid',
        gridTemplateColumns: '260px 1fr',
        overflow: 'hidden',
      }}
    >
      <TreePanel />
      <Viewport />
    </div>
  );
}
