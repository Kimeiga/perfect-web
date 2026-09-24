import { compileWorldPrompt, type WorldIR } from "./world";

type Progress = (message: string) => void;

type Atmosphere = "neutral" | "void" | "flood" | "machine";

interface Intent {
  atmosphere: Atmosphere;
  fogDensity: number;
  pointSize: number;
  depthScale: number;
  drift: number;
  pulse: number;
  audioGain: number;
  exposure: number;
  spin: number;
}

const schema = {
  type: "object",
  properties: {
    atmosphere: { enum: ["neutral", "void", "flood", "machine"] },
    fogDensity: { type: "number", minimum: 0.005, maximum: 0.12 },
    pointSize: { type: "number", minimum: 0.015, maximum: 0.07 },
    depthScale: { type: "number", minimum: 2.5, maximum: 14 },
    drift: { type: "number", minimum: 0, maximum: 0.8 },
    pulse: { type: "number", minimum: 0, maximum: 0.4 },
    audioGain: { type: "number", minimum: 0, maximum: 1.8 },
    exposure: { type: "number", minimum: 0.65, maximum: 2 },
    spin: { type: "number", minimum: 0, maximum: 0.22 },
  },
  required: [
    "atmosphere",
    "fogDensity",
    "pointSize",
    "depthScale",
    "drift",
    "pulse",
    "audioGain",
    "exposure",
    "spin",
  ],
  additionalProperties: false,
} as const;

const palettes: Record<Atmosphere, Pick<WorldIR, "background" | "fog">> = {
  neutral: { background: "#050608", fog: "#0b1016" },
  void: { background: "#010103", fog: "#080a12" },
  flood: { background: "#02070d", fog: "#061b2a" },
  machine: { background: "#090201", fog: "#210603" },
};

let generatorPromise: Promise<any> | undefined;

async function generator(progress: Progress) {
  if (generatorPromise) return generatorPromise;
  generatorPromise = (async () => {
    progress("Loading local intent model · first use only…");
    const { pipeline } = await import("@huggingface/transformers");
    return pipeline(
      "text-generation",
      "onnx-community/LFM2.5-350M-ONNX",
      {
        dtype: "q4f16",
        device: "webgpu",
        progress_callback: (event: { status?: string; progress?: number }) => {
          if (event.status === "progress" && typeof event.progress === "number")
            progress("Caching local intent model · " + Math.round(event.progress) + "%");
        },
      },
    );
  })();
  return generatorPromise;
}

function worldFromIntent(intent: Intent): WorldIR {
  const palette = palettes[intent.atmosphere] ?? palettes.neutral;
  return {
    ...palette,
    fogDensity: intent.fogDensity,
    pointSize: intent.pointSize,
    depthScale: intent.depthScale,
    drift: intent.drift,
    pulse: intent.pulse,
    audioGain: intent.audioGain,
    exposure: intent.exposure,
    spin: intent.spin,
  };
}

function generatedContent(output: any): string {
  const generated = output?.[0]?.generated_text;
  if (Array.isArray(generated)) {
    const content = generated.at(-1)?.content;
    if (typeof content === "string") return content;
  }
  if (typeof generated === "string") return generated;
  throw new Error("Local intent model returned no structured world.");
}

/**
 * Compile language to a bounded WorldIR with a tiny local model.
 * The deterministic compiler remains the fallback on non-WebGPU browsers,
 * failed model loads, or malformed output.
 */
export async function compileWorldIntent(
  prompt: string,
  progress: Progress,
): Promise<{ world: WorldIR; source: "local-ai" | "deterministic" }> {
  if (!("gpu" in navigator))
    return { world: compileWorldPrompt(prompt), source: "deterministic" };

  try {
    const run = await generator(progress);
    const { StructuredOutputProcessor } =
      await import("@huggingface/transformers-structured-output");
    const processor = new StructuredOutputProcessor(run.tokenizer, {
      type: "json_schema",
      json_schema: schema,
    });
    progress("Compiling intent locally…");
    const output = await run(
      [
        {
          role: "system",
          content:
            "Translate the user's visual direction into one bounded interactive world program. " +
            "Use atmosphere for the overall material world. Higher depthScale exaggerates spatial depth. " +
            "drift/spin control ambient motion, pulse controls rhythmic deformation, and audioGain controls microphone reactivity. " +
            "Prefer restrained values unless the user explicitly asks for violent, extreme, dense, fast, or dramatic behavior.",
        },
        { role: "user", content: prompt.slice(0, 1200) },
      ],
      {
        max_new_tokens: 180,
        do_sample: false,
        logits_processor: [processor],
      },
    );
    const intent = JSON.parse(generatedContent(output)) as Intent;
    return { world: worldFromIntent(intent), source: "local-ai" };
  } catch (error) {
    console.warn("Local intent compilation failed; using deterministic compiler.", error);
    return { world: compileWorldPrompt(prompt), source: "deterministic" };
  }
}
