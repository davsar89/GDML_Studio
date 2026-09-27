import TreePanel from './TreePanel/TreePanel';
import Viewport from './Viewport/Viewport';

export default function Layout() {
  return (
    <div
      style={{
        flex: 1,
        minHeight: 0, // shrink below content height when a banner takes space, instead of overflowing
        display: 'grid',
        gridTemplateColumns: '260px 1fr',
        gridTemplateRows: 'minmax(0, 1fr)',
        overflow: 'hidden',
      }}
    >
      <TreePanel />
      <Viewport />
    </div>
  );
}
