import { useCallback } from 'react';

interface Props {
  loading: boolean;
  error: string | null;
  onFile: (file: File) => void;
}

export function FileLoader({ loading, error, onFile }: Props) {
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) onFile(file);
    },
    [onFile],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const file = e.dataTransfer.files[0];
      if (file) onFile(file);
    },
    [onFile],
  );

  return (
    <div
      onDrop={handleDrop}
      onDragOver={(e) => e.preventDefault()}
      style={{
        border: '2px dashed #888',
        borderRadius: 8,
        padding: 40,
        textAlign: 'center',
        cursor: 'pointer',
      }}
    >
      {loading ? (
        <p>Loading snapshot...</p>
      ) : (
        <>
          <p>Drop a .heapsnapshot file here, or click to select</p>
          <input
            type="file"
            accept=".heapsnapshot"
            onChange={handleChange}
            style={{ marginTop: 8 }}
          />
        </>
      )}
      {error && <p style={{ color: 'red', marginTop: 8 }}>{error}</p>}
    </div>
  );
}
