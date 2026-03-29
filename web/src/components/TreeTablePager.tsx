import { useState, useCallback } from 'react';

const PAGE_SIZE = 100;

interface Props {
  depth: number;
  shown: number;
  total: number;
  offset: number;
  filter: string;
  onPageChange: (offset: number, limit: number) => void;
  onFilterChange: (filter: string) => void;
  onShowAll: () => void;
}

const btnStyle: React.CSSProperties = {
  padding: '1px 6px',
  fontSize: 11,
  cursor: 'pointer',
  border: '1px solid #ccc',
  borderRadius: 3,
  background: '#fafafa',
};

const inputStyle: React.CSSProperties = {
  width: 50,
  padding: '1px 4px',
  fontSize: 11,
  textAlign: 'center',
  border: '1px solid #ccc',
  borderRadius: 3,
};

export function TreeTablePager({
  depth,
  shown,
  total,
  offset,
  filter,
  onPageChange,
  onFilterChange,
  onShowAll,
}: Props) {
  const [filterInput, setFilterInput] = useState(filter);
  const [startInput, setStartInput] = useState(String(offset + 1));
  const [endInput, setEndInput] = useState(String(offset + shown));

  const end = offset + shown;
  const limit = shown;

  const applyRange = useCallback(() => {
    const s = Math.max(1, parseInt(startInput, 10) || 1) - 1;
    const e = Math.max(s + 1, parseInt(endInput, 10) || s + PAGE_SIZE);
    onPageChange(s, e - s);
  }, [startInput, endInput, onPageChange]);

  const handleFilterSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      onFilterChange(filterInput);
    },
    [filterInput, onFilterChange],
  );

  if (total <= PAGE_SIZE && !filter) return null;

  return (
    <tr>
      <td
        colSpan={7}
        style={{
          padding: '4px 8px',
          paddingLeft: 8 + depth * 16,
          fontSize: 11,
        }}
      >
        <span
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 4,
            flexWrap: 'wrap',
          }}
        >
          <input
            style={inputStyle}
            value={startInput}
            onChange={(e) => setStartInput(e.target.value)}
            onBlur={applyRange}
            onKeyDown={(e) => e.key === 'Enter' && applyRange()}
            onClick={(e) => e.stopPropagation()}
          />
          <span style={{ color: '#888' }}>&ndash;</span>
          <input
            style={inputStyle}
            value={endInput}
            onChange={(e) => setEndInput(e.target.value)}
            onBlur={applyRange}
            onKeyDown={(e) => e.key === 'Enter' && applyRange()}
            onClick={(e) => e.stopPropagation()}
          />
          <span style={{ color: '#888' }}>of {total}</span>

          <button
            style={btnStyle}
            disabled={offset === 0}
            onClick={(e) => {
              e.stopPropagation();
              const newOffset = Math.max(0, offset - limit);
              onPageChange(newOffset, limit);
              setStartInput(String(newOffset + 1));
              setEndInput(String(newOffset + limit));
            }}
          >
            &larr; Prev
          </button>
          <button
            style={btnStyle}
            disabled={end >= total}
            onClick={(e) => {
              e.stopPropagation();
              const newOffset = Math.min(total - 1, offset + limit);
              const newEnd = Math.min(total, newOffset + limit);
              onPageChange(newOffset, newEnd - newOffset);
              setStartInput(String(newOffset + 1));
              setEndInput(String(newEnd));
            }}
          >
            Next &rarr;
          </button>
          <button
            style={btnStyle}
            disabled={limit >= total}
            onClick={(e) => {
              e.stopPropagation();
              onShowAll();
              setStartInput('1');
              setEndInput(String(total));
            }}
          >
            All
          </button>
          <button
            style={btnStyle}
            disabled={limit <= PAGE_SIZE}
            onClick={(e) => {
              e.stopPropagation();
              const newLimit = Math.max(PAGE_SIZE, limit - PAGE_SIZE);
              onPageChange(offset, newLimit);
              setEndInput(String(offset + newLimit));
            }}
          >
            &minus;100
          </button>
          <button
            style={btnStyle}
            disabled={offset + limit >= total}
            onClick={(e) => {
              e.stopPropagation();
              const newLimit = Math.min(total - offset, limit + PAGE_SIZE);
              onPageChange(offset, newLimit);
              setEndInput(String(offset + newLimit));
            }}
          >
            +100
          </button>

          <form
            onSubmit={handleFilterSubmit}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 4,
              marginLeft: 8,
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <input
              style={{ ...inputStyle, width: 120, textAlign: 'left' }}
              value={filterInput}
              onChange={(e) => setFilterInput(e.target.value)}
              placeholder="Filter edges..."
            />
            {filterInput !== filter && (
              <button type="submit" style={btnStyle}>
                Apply
              </button>
            )}
            {filter && (
              <button
                type="button"
                style={btnStyle}
                onClick={() => {
                  setFilterInput('');
                  onFilterChange('');
                }}
              >
                Clear
              </button>
            )}
          </form>
        </span>
      </td>
    </tr>
  );
}
