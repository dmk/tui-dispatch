import { TuiPreview } from "@dkkoval/tui-preview";

const terminalTheme = {
  background: "#0b1020",
  foreground: "#dce6ff",
  cursor: "#7aa2f7",
  selectionBackground: "#223355",
  selectionForeground: "#dce6ff",
};

const baseUrl = import.meta.env.BASE_URL.endsWith("/")
  ? import.meta.env.BASE_URL
  : `${import.meta.env.BASE_URL}/`;

interface ExamplePreviewProps {
  name: string;
  height?: number;
}

export function ExamplePreview({ name, height = 360 }: ExamplePreviewProps) {
  const wasmUrl = `${baseUrl}wasm/preview-${name}.wasm`;

  return (
    <div
      style={{
        width: "100%",
        height: `${height}px`,
        borderRadius: 10,
        overflow: "hidden",
        border:
          "1px solid color-mix(in srgb, var(--sl-color-gray-4), transparent 35%)",
      }}
    >
      <TuiPreview
        wasm={wasmUrl}
        mode="interactive"
        fit="container"
        terminal={{
          fontSize: 12,
          fontFamily:
            "Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
          theme: terminalTheme,
        }}
        style={{ width: "100%", height: "100%" }}
      />
    </div>
  );
}
