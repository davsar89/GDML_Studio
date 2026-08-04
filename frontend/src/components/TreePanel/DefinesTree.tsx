import { useEffect, useRef, useState } from 'react';
import { useAppStore } from '../../store';

/**
 * Row geometry. Both are pinned rather than left to the browser so the
 * scroll-offset arithmetic below is exact: a fractional line-height would drift
 * the spacer against the rendered rows over thousands of entries.
 */
const ROW_HEIGHT = 16;
/** Extra rows above and below the viewport, so a fast scroll shows no gap. */
const OVERSCAN = 8;

/**
 * Windowed list of the document's defines.
 *
 * Every define used to be a DOM node. Sizing measured against the shipped
 * corpus, not guessed: `/api/document/defines` returns only the SCALAR defines
 * — constants, quantities, variables, expressions — so the largest sample
 * (`NaiDetModelWithMLI`) shows 400 rows and `pod_asm_tessellated` shows none at
 * all, its 22,222 `<position>` entries being vertex data this endpoint omits.
 *
 * 400 rows does not need windowing. This exists to bound the cost for user
 * files larger than anything shipped; on the corpus itself it is a wash, and it
 * does add a re-render per scroll event that plain rendering did not have.
 *
 * Hand-rolled rather than pulling in a virtualisation library: the list is flat
 * and every row is the same height, which is the one case where the arithmetic
 * is trivial. The structure tree is NOT virtualised — it is nested, and the
 * largest sample expands to 164 nodes, so there is nothing there to win.
 */
export default function DefinesTree() {
  const defines = useAppStore((s) => s.defines);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    setViewportHeight(el.clientHeight);
    // The panel is a flex child, so it resizes without a window resize event.
    const observer = new ResizeObserver(() => setViewportHeight(el.clientHeight));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  if (defines.length === 0) {
    return <div style={{ color: '#666', fontSize: 12, padding: 8 }}>No file loaded</div>;
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
                {d.evaluated !== null ? d.evaluated.toFixed(4) : '?'}
              </span>
              {d.unit && <span style={{ color: '#666' }}> {d.unit}</span>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
