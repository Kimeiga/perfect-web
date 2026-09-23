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

const includesAny = (text: string, words: string[]) =>
  words.some((word) => text.includes(word));

export function compileWorldPrompt(prompt: string): WorldIR {
  const text = prompt.toLowerCase();
  const world = { ...DEFAULT_WORLD };

  if (includesAny(text, ["flood", "water", "ocean", "underwater", "blue"])) {
    Object.assign(world, {
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
    });
  }

  if (includesAny(text, ["void", "space", "lunar", "moon", "cold", "dust"])) {
    Object.assign(world, {
      background: "#010103",
      fog: "#080a12",
      fogDensity: 0.018,
      pointSize: 0.027,
      depthScale: 7.8,
      drift: 0.11,
      pulse: 0.05,
      exposure: 1.45,
      spin: 0.03,
    });
  }

  if (includesAny(text, ["machine", "industrial", "hot", "red", "violent", "inferno"])) {
    Object.assign(world, {
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
    });
  }

  if (includesAny(text, ["calm", "still", "quiet", "minimal"])) {
    world.drift *= 0.3;
    world.pulse *= 0.25;
    world.spin *= 0.3;
    world.fogDensity *= 0.6;
  }

  if (includesAny(text, ["extreme depth", "deep", "dramatic depth"])) world.depthScale *= 1.45;
  if (includesAny(text, ["dense", "fog", "atmosphere"])) world.fogDensity *= 1.35;
  if (includesAny(text, ["bass", "sound", "audio", "music"])) world.audioGain *= 1.35;

  return world;
}
