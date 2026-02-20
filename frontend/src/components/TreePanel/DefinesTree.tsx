import { useAppStore } from '../../store';

export default function DefinesTree() {
  const defines = useAppStore((s) => s.defines);

  if (defines.length === 0) {
    return <div style={{ color: '#666', fontSize: 12 }}>No file loaded</div>;
  }

  return (
    <div>
      {defines.map((d) => (
        <div
          key={d.name}
          style={{
            fontSize: 11,
            fontFamily: 'monospace',
            padding: '1px 4px',
            color: '#b0b8c0',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
          title={`${d.name} = ${d.expression} => ${d.evaluated}`}
        >
          <span style={{ color: '#e94560' }}>{d.name}</span>
          <span style={{ color: '#666' }}> = </span>
          <span style={{ color: '#4fc3f7' }}>
            {d.evaluated !== null ? d.evaluated.toFixed(4) : '?'}
          </span>
          {d.unit && <span style={{ color: '#666' }}> {d.unit}</span>}
        </div>
      ))}
    </div>
  );
}
