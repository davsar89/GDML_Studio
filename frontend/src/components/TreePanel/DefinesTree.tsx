import { useEffect, useRef, useState } from 'react';
import { useAppStore } from '../../store';

// Windowed list of the scalar defines, bounding the DOM for files with thousands of them.
// Rows have a pinned height so the scroll offset maps exactly to a row index;
// OVERSCAN extra rows above and below keep a fast scroll from showing a gap.
const ROW_HEIGHT = 32;
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
    return <div className="empty-state">{loaded ? 'No defines' : 'No file loaded'}</div>;
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
      className="panel-scroll"
      data-testid="defines-scroll"
    >
      {/* One tall spacer holds the scrollbar; only `rows` are ever mounted. */}
      <div style={{ height: total * ROW_HEIGHT, position: 'relative' }}>
        <div style={{ position: 'absolute', top: start * ROW_HEIGHT, left: 0, right: 0 }}>
          {rows.map((d) => (
            <div
              key={d.name}
              className="define-row"
              style={{ height: ROW_HEIGHT }}
              title={`${d.name} = ${d.expression} => ${d.evaluated}`}
            >
              <span className="truncate">{d.name}</span>
              <span className="define-value">
                {/* Six significant digits: fixed decimals would show 1e-9 as 0. */}
                {d.evaluated !== null ? String(Number(d.evaluated.toPrecision(6))) : '?'}
              </span>
              {d.unit && <span className="muted">{d.unit}</span>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
