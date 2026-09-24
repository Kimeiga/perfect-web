export interface DepthField {
  width: number;
  height: number;
  depth: Float32Array;
  rgba: Uint8ClampedArray;
  backend: "webgpu" | "wasm";
}

type Progress = (message: string) => void;
type Estimator = { run: any; backend: "webgpu" | "wasm" };

let estimatorPromise: Promise<Estimator> | undefined;

function reportDownload(progress: Progress) {
  return (event: { status?: string; progress?: number }) => {
    if (event.status !== "progress" || typeof event.progress !== "number") return;
    progress("Caching local model · " + Math.round(event.progress) + "%");
  };
}

async function wasmEstimator(pipeline: any, model: string, progress: Progress): Promise<Estimator> {
  progress("Loading local depth model on WASM…");
  const run = await pipeline("depth-estimation", model, {
    progress_callback: reportDownload(progress),
  });
  return { run, backend: "wasm" };
}

async function webgpuEstimator(pipeline: any, model: string, progress: Progress): Promise<Estimator | null> {
  if (!("gpu" in navigator)) return null;
  try {
    progress("Loading local depth model on WebGPU…");
    const run = await pipeline("depth-estimation", model, {
      device: "webgpu",
      progress_callback: reportDownload(progress),
    });
    return { run, backend: "webgpu" };
  } catch (error) {
    console.warn("WebGPU depth inference unavailable; falling back to WASM.", error);
    return null;
  }
}

async function createEstimator(progress: Progress): Promise<Estimator> {
  const { pipeline } = await import("@huggingface/transformers");
  const model = "onnx-community/depth-anything-v2-small";
  return (await webgpuEstimator(pipeline, model, progress))
    ?? wasmEstimator(pipeline, model, progress);
}

function normalizedDepth(raw: { data: ArrayLike<number>; width: number; height: number }) {
  const { width, height, data } = raw;
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
  return depth;
}

async function sourcePixels(file: File, width: number, height: number) {
  const bitmap = await createImageBitmap(file);
  try {
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("Canvas 2D context unavailable.");
    context.drawImage(bitmap, 0, 0, width, height);
    return context.getImageData(0, 0, width, height).data;
  } finally {
    bitmap.close();
  }
}

function positiveFinite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function validRawDepth(raw: any): raw is { data: ArrayLike<number>; width: number; height: number } {
  if (!raw?.data) return false;
  return positiveFinite(raw.width) && positiveFinite(raw.height);
}

async function runDepth(file: File, estimator: Estimator, progress: Progress) {
  const url = URL.createObjectURL(file);
  try {
    progress("Perceiving depth locally · " + estimator.backend.toUpperCase());
    return await estimator.run(url);
  } finally {
    URL.revokeObjectURL(url);
  }
}

export async function inferDepth(file: File, progress: Progress): Promise<DepthField> {
  estimatorPromise ??= createEstimator(progress);
  const estimator = await estimatorPromise;
  const result = await runDepth(file, estimator, progress);
  const raw = result.depth;
  if (!validRawDepth(raw))
    throw new Error("Depth model returned no renderable depth field.");

  const depth = normalizedDepth(raw);
  const rgba = await sourcePixels(file, raw.width, raw.height);
  progress("Depth field ready.");
  return {
    width: raw.width,
    height: raw.height,
    depth,
    rgba,
    backend: estimator.backend,
  };
}
