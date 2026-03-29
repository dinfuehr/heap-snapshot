import { createSignal, Show, type JSX } from 'solid-js';

const PAGE_SIZE = 100;

const btnStyle = {
  padding: '1px 6px',
  'font-size': '11px',
  cursor: 'pointer',
  border: '1px solid #ccc',
  'border-radius': '3px',
  background: '#fafafa',
};

const inputStyle = {
  width: '50px',
  padding: '1px 4px',
  'font-size': '11px',
  'text-align': 'center' as const,
  border: '1px solid #ccc',
  'border-radius': '3px',
};

export function TreeTablePager(props: {
  depth: number;
  shown: number;
  total: number;
  offset: number;
  filter: string;
  onPageChange: (offset: number, limit: number) => void;
  onFilterChange: (filter: string) => void;
  onShowAll: () => void;
}): JSX.Element {
  const [filterInput, setFilterInput] = createSignal(props.filter);
  const [startInput, setStartInput] = createSignal(String(props.offset + 1));
  const [endInput, setEndInput] = createSignal(
    String(props.offset + props.shown),
  );

  const limit = () => props.shown;
  const hidden = () => props.total <= PAGE_SIZE && !props.filter;

  const applyRange = () => {
    const s = Math.max(1, parseInt(startInput(), 10) || 1) - 1;
    const e = Math.max(s + 1, parseInt(endInput(), 10) || s + PAGE_SIZE);
    props.onPageChange(s, e - s);
  };

  const handleFilterSubmit = (e: Event) => {
    e.preventDefault();
    props.onFilterChange(filterInput());
  };

  return (
    <Show when={!hidden()}>
      <tr>
        <td
          colSpan={6}
          style={{
            padding: '4px 8px',
            'padding-left': `${8 + props.depth * 16}px`,
            'font-size': '11px',
          }}
        >
          <span
            style={{
              display: 'inline-flex',
              'align-items': 'center',
              gap: '4px',
              'flex-wrap': 'wrap',
            }}
          >
            <input
              style={inputStyle}
              value={startInput()}
              onInput={(e) => setStartInput(e.currentTarget.value)}
              onBlur={applyRange}
              onKeyDown={(e) => e.key === 'Enter' && applyRange()}
              onClick={(e) => e.stopPropagation()}
            />
            <span style={{ color: '#888' }}>&ndash;</span>
            <input
              style={inputStyle}
              value={endInput()}
              onInput={(e) => setEndInput(e.currentTarget.value)}
              onBlur={applyRange}
              onKeyDown={(e) => e.key === 'Enter' && applyRange()}
              onClick={(e) => e.stopPropagation()}
            />
            <span style={{ color: '#888' }}>of {props.total}</span>

            <button
              style={btnStyle}
              disabled={props.offset === 0}
              onClick={(e) => {
                e.stopPropagation();
                const o = Math.max(0, props.offset - limit());
                props.onPageChange(o, limit());
                setStartInput(String(o + 1));
                setEndInput(String(o + limit()));
              }}
            >
              &larr; Prev
            </button>
            <button
              style={btnStyle}
              disabled={props.offset + props.shown >= props.total}
              onClick={(e) => {
                e.stopPropagation();
                const o = Math.min(props.total - 1, props.offset + limit());
                const end = Math.min(props.total, o + limit());
                props.onPageChange(o, end - o);
                setStartInput(String(o + 1));
                setEndInput(String(end));
              }}
            >
              Next &rarr;
            </button>
            <button
              style={btnStyle}
              disabled={limit() >= props.total}
              onClick={(e) => {
                e.stopPropagation();
                props.onShowAll();
                setStartInput('1');
                setEndInput(String(props.total));
              }}
            >
              All
            </button>
            <button
              style={btnStyle}
              disabled={limit() <= PAGE_SIZE}
              onClick={(e) => {
                e.stopPropagation();
                const nl = Math.max(PAGE_SIZE, limit() - PAGE_SIZE);
                props.onPageChange(props.offset, nl);
                setEndInput(String(props.offset + nl));
              }}
            >
              &minus;100
            </button>
            <button
              style={btnStyle}
              disabled={props.offset + limit() >= props.total}
              onClick={(e) => {
                e.stopPropagation();
                const nl = Math.min(
                  props.total - props.offset,
                  limit() + PAGE_SIZE,
                );
                props.onPageChange(props.offset, nl);
                setEndInput(String(props.offset + nl));
              }}
            >
              +100
            </button>

            <form
              onSubmit={handleFilterSubmit}
              style={{
                display: 'inline-flex',
                'align-items': 'center',
                gap: '4px',
                'margin-left': '8px',
              }}
              onClick={(e) => e.stopPropagation()}
            >
              <input
                style={{ ...inputStyle, width: '120px', 'text-align': 'left' }}
                value={filterInput()}
                onInput={(e) => setFilterInput(e.currentTarget.value)}
                placeholder="Filter edges..."
              />
              {filterInput() !== props.filter && (
                <button type="submit" style={btnStyle}>
                  Apply
                </button>
              )}
              {props.filter && (
                <button
                  type="button"
                  style={btnStyle}
                  onClick={() => {
                    setFilterInput('');
                    props.onFilterChange('');
                  }}
                >
                  Clear
                </button>
              )}
            </form>
          </span>
        </td>
      </tr>
    </Show>
  );
}
