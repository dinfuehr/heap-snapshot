import type { JSX } from 'solid-js';

export function FileLoader(props: {
  loading: boolean;
  error: string | null;
  onFile: (file: File) => void;
}): JSX.Element {
  const handleChange = (e: Event) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) props.onFile(file);
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    const file = e.dataTransfer?.files[0];
    if (file) props.onFile(file);
  };

  return (
    <div
      onDrop={handleDrop}
      onDragOver={(e) => e.preventDefault()}
      style={{
        border: '2px dashed #888',
        'border-radius': '8px',
        padding: '40px',
        'text-align': 'center',
        cursor: 'pointer',
      }}
    >
      {props.loading ? (
        <p>Loading snapshot...</p>
      ) : (
        <>
          <p>Drop a .heapsnapshot file here, or click to select</p>
          <input
            type="file"
            accept=".heapsnapshot"
            onChange={handleChange}
            style={{ 'margin-top': '8px' }}
          />
        </>
      )}
      {props.error && (
        <p style={{ color: 'red', 'margin-top': '8px' }}>{props.error}</p>
      )}
    </div>
  );
}
