// Benchmark instrumentation, not application runtime.
// forcedStyleAndLayoutDuration belongs to PerformanceScriptTiming, not a frame.
// https://w3c.github.io/long-animation-frames/#sec-PerformanceScriptTiming
// Report only attributed scripts. Missing observations are not observed zeroes,
// and even a reported zero cannot prove absence of layout in shorter frames.

const duration = (value) =>
  typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
const text = (value) => typeof value === "string" ? value : null;

export function serializeLongAnimationFrame(entry) {
  if (entry === null || typeof entry !== "object") {
    throw new TypeError("Expected a Long Animation Frame entry");
  }
  const scriptsAvailable = Array.isArray(entry.scripts);
  return {
    startTime: duration(entry.startTime),
    duration: duration(entry.duration),
    blockingDuration: duration(entry.blockingDuration),
    renderStart: duration(entry.renderStart),
    styleAndLayoutStart: duration(entry.styleAndLayoutStart),
    scriptsAvailable,
    scripts: scriptsAvailable ? entry.scripts.map((script) => ({
      duration: duration(script?.duration),
      forcedStyleAndLayoutDuration: duration(script?.forcedStyleAndLayoutDuration),
      invokerType: text(script?.invokerType),
      sourceURL: text(script?.sourceURL),
      sourceFunctionName: text(script?.sourceFunctionName),
    })) : [],
  };
}

export function summarizeLayoutAttribution(frames) {
  if (!Array.isArray(frames)) throw new TypeError("Expected an array of recorded frames");
  let reportedScripts = 0;
  let missingScripts = 0;
  let framesWithoutScripts = 0;
  let sum = 0;
  for (const frame of frames) {
    if (!Array.isArray(frame?.scripts) || frame.scripts.length === 0) {
      framesWithoutScripts++;
      continue;
    }
    for (const script of frame.scripts) {
      const value = duration(script?.forcedStyleAndLayoutDuration);
      if (value === null) {
        missingScripts++;
      } else {
        reportedScripts++;
        sum += value;
      }
    }
  }
  return {
    state: reportedScripts === 0 ? "unobserved"
      : missingScripts > 0 || framesWithoutScripts > 0 ? "partial" : "observed",
    frameCount: frames.length,
    reportedScripts,
    missingScripts,
    framesWithoutScripts,
    // This is a subtotal over reported script records, not total page layout.
    reportedForcedStyleAndLayoutDuration: reportedScripts > 0 ? sum : null,
  };
}

export function installLayoutObserver(target = globalThis) {
  target.__loaf = [];
  target.__loafSupported = false;
  target.__loafObservation = "unsupported";
  target.__loafError = null;
  const Observer = target.PerformanceObserver;
  if (!Observer?.supportedEntryTypes?.includes("long-animation-frame")) return null;
  const observer = new Observer((list) => {
    for (const entry of list.getEntries()) {
      target.__loaf.push(serializeLongAnimationFrame(entry));
    }
  });
  try {
    observer.observe({ type: "long-animation-frame", buffered: true });
  } catch (error) {
    observer.disconnect();
    target.__loafObservation = "failed";
    target.__loafError = error instanceof Error ? error.message : String(error);
    throw error;
  }
  target.__loafSupported = true;
  target.__loafObservation = "observing";
  return observer;
}
