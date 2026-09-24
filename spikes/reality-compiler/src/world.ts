export interface WorldIR {
  background: string;
  fog: string;
  fogDensity: number;
  pointSize: number;
  depthScale: number;
  drift: number;
  pulse: number;
  audioGain: number;
  exposure: number;
  spin: number;
}

export const DEFAULT_WORLD: WorldIR = {
  background: "#050608",
  fog: "#050608",
  fogDensity: 0.03,
  pointSize: 0.032,
  depthScale: 5.8,
  drift: 0.16,
  pulse: 0.08,
  audioGain: 0.65,
  exposure: 1.15,
  spin: 0.055,
};

type WorldPatch = Partial<WorldIR>;
type NumericWorldKey =
  | "fogDensity"
  | "depthScale"
  | "drift"
  | "pulse"
  | "audioGain"
  | "spin";

const PRESETS: { words: string[]; patch: WorldPatch }[] = [
  {
    words: ["flood", "water", "ocean", "underwater", "blue"],
    patch: {
      background: "#02070d",
      fog: "#061b2a",
      fogDensity: 0.055,
      pointSize: 0.038,
      depthScale: 6.8,
      drift: 0.32,
      pulse: 0.12,
      audioGain: 0.9,
      exposure: 1.3,
      spin: 0.075,
    },
  },
  {
    words: ["void", "space", "lunar", "moon", "cold", "dust"],
    patch: {
      background: "#010103",
      fog: "#080a12",
      fogDensity: 0.018,
      pointSize: 0.027,
      depthScale: 7.8,
      drift: 0.11,
      pulse: 0.05,
      exposure: 1.45,
      spin: 0.03,
    },
  },
  {
    words: ["machine", "industrial", "hot", "red", "violent", "inferno"],
    patch: {
      background: "#090201",
      fog: "#210603",
      fogDensity: 0.07,
      pointSize: 0.043,
      depthScale: 5.1,
      drift: 0.5,
      pulse: 0.22,
      audioGain: 1.1,
      exposure: 1.55,
      spin: 0.12,
    },
  },
];

const MULTIPLIERS: {
  words: string[];
  factors: Partial<Record<NumericWorldKey, number>>;
}[] = [
  {
    words: ["calm", "still", "quiet", "minimal"],
    factors: { drift: 0.3, pulse: 0.25, spin: 0.3, fogDensity: 0.6 },
  },
  {
    words: ["extreme depth", "deep", "dramatic depth"],
    factors: { depthScale: 1.45 },
  },
  {
    words: ["dense", "fog", "atmosphere"],
    factors: { fogDensity: 1.35 },
  },
  {
    words: ["bass", "sound", "audio", "music"],
    factors: { audioGain: 1.35 },
  },
];

const includesAny = (text: string, words: string[]) =>
  words.some((word) => text.includes(word));

function applyPresets(world: WorldIR, text: string) {
  for (const preset of PRESETS)
    if (includesAny(text, preset.words)) Object.assign(world, preset.patch);
}

function applyFactors(world: WorldIR, factors: Partial<Record<NumericWorldKey, number>>) {
  for (const [key, factor] of Object.entries(factors) as [NumericWorldKey, number][])
    world[key] *= factor;
}

function applyModifiers(world: WorldIR, text: string) {
  for (const modifier of MULTIPLIERS)
    if (includesAny(text, modifier.words)) applyFactors(world, modifier.factors);
}

export function compileWorldPrompt(prompt: string): WorldIR {
  const text = prompt.toLowerCase();
  const world = { ...DEFAULT_WORLD };
  applyPresets(world, text);
  applyModifiers(world, text);
  return world;
}
