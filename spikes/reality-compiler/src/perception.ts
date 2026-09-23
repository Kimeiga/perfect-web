export interface DepthField {
  width: number;
  height: number;
  depth: Float32Array;
  rgba: Uint8ClampedArray;
  backend: "webgpu" | "wasm";
}

type Progress = (message: string) => void;

let estimatorPromise: Promise<{ run: any; backend: "webgpu" | "wasm" }> | undefined;

async function createEstimator(progress: Progress) {
  const { pipeline } = await import("@huggingface/transformers");
  const model = "onnx-community/depth-anything-v2-small";

  if ("gpu" in navigator) {
    try {
      progress("Loading local depth model on WebGPU…");
      const run = await pipeline("depth-estimation", model, {
        device: "webgpu",
        progress_callback: (event: { status?: string; progress?: number }) => {
          if (event.status === "progress" && typeof event.progress === "number") {
            progress("Caching local model · " + Math.round(event.progress) + "%");
          }
        },
      });
      return { run, backend: "webgpu" as const };
    } catch (error) {
      console.warn("WebGPU depth inference unavailable; falling back to WASM.", error);
    }
  }

  progress("Loading local depth model on WASM…");
  const run = await pipeline("depth-estimation", model, {
    progress_callback: (event: { status?: string; progress?: number }) => {
      if (event.status === "progress" && typeof event.progress === "number") {
        progress("Caching local model · " + Math.round(event.progress) + "%");
      }
    },
  });
  return { run, backend: "wasm" as const };
}

export async function inferDepth(file: File, progress: Progress): Promise<DepthField> {
  estimatorPromise ??= createEstimator(progress);
  const estimator = await estimatorPromise;
  const url = URL.createObjectURL(file);

  try {
    progress("Perceiving depth locally · " + estimator.backend.toUpperCase());
    const result = await estimator.run(url);
    const raw = result.depth;
    if (!raw?.data || !raw.width || !raw.height) {
      throw new Error("Depth model returned no renderable depth field.");
    }

    const width = raw.width as number;
    const height = raw.height as number;
    const data = raw.data as ArrayLike<number>;
    const channels = Math.max(1, Math.round(data.length / (width * height)));
    const depth = new Float32Array(width * height);

    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (let i = 0; i < depth.length; i++) {
      const value = Number(data[i * channels]);
      depth[i] = value;
      min = Math.min(min, value);
      max = Math.max(max, value);
    }
    const span = Math.max(1e-6, max - min);
    for (let i = 0; i < depth.length; i++) depth[i] = (depth[i] - min) / span;

    const bitmap = await createImageBitmap(file);
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("Canvas 2D context unavailable.");
    context.drawImage(bitmap, 0, 0, width, height);
    bitmap.close();

    const rgba = context.getImageData(0, 0, width, height).data;
    progress("Depth field ready.");
    return { width, height, depth, rgba, backend: estimator.backend };
  } finally {
    URL.revokeObjectURL(url);
  }
}
