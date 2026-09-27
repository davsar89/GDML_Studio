import { useEffect, useRef, useState } from 'react';
import { useAppStore } from '../../store';

// Windowed list of the scalar defines, bounding the DOM for files with thousands of them.
// Rows have a pinned height so the scroll offset maps exactly to a row index;
// OVERSCAN extra rows above and below keep a fast scroll from showing a gap.
const ROW_HEIGHT = 16;
const OVERSCAN = 8;

export default function DefinesTree() {
  const defines = useAppStore((s) => s.defines);
  const loaded = useAppStore((s) => s.summary !== null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const hasRows = defines.length > 0;

  // The scroller only mounts with the first rows, which may arrive after this component.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    setViewportHeight(el.clientHeight);
    // The panel is a flex child, so it resizes without a window resize event.
    const observer = new ResizeObserver(() => setViewportHeight(el.clientHeight));
    observer.observe(el);
    return () => observer.disconnect();
  }, [hasRows]);

  if (!hasRows) {
    return <div style={{ color: '#666', fontSize: 12, padding: 8 }}>{loaded ? 'No defines' : 'No file loaded'}</div>;
  }

  const total = defines.length;
  // viewportHeight is 0 on the very first render, before the effect measures it.
  // Fall back to a screenful so that pass is not blank.
  const visibleCount = Math.ceil((viewportHeight || 600) / ROW_HEIGHT) + OVERSCAN * 2;
  const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const end = Math.min(total, start + visibleCount);
  const rows = defines.slice(start, end);

  return (
    <div
      ref={scrollRef}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      style={{ flex: 1, minHeight: 0, overflow: 'auto', padding: 8 }}
      data-testid="defines-scroll"
    >
      {/* One tall spacer holds the scrollbar; only `rows` are ever mounted. */}
      <div style={{ height: total * ROW_HEIGHT, position: 'relative' }}>
        <div style={{ position: 'absolute', top: start * ROW_HEIGHT, left: 0, right: 0 }}>
          {rows.map((d) => (
            <div
              key={d.name}
              style={{
                height: ROW_HEIGHT,
                lineHeight: `${ROW_HEIGHT - 2}px`,
                boxSizing: 'border-box',
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
                {/* Six significant digits: fixed decimals would show 1e-9 as 0. */}
                {d.evaluated !== null ? String(Number(d.evaluated.toPrecision(6))) : '?'}
              </span>
              {d.unit && <span style={{ color: '#666' }}> {d.unit}</span>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
