import * as THREE from "three/webgpu";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { compileWorldPrompt, DEFAULT_WORLD, type WorldIR } from "./world";
import { inferDepth, type DepthField } from "./perception";
import { compileWorldIntent } from "./intent";

const canvas = document.querySelector<HTMLCanvasElement>("#world")!;
const status = document.querySelector<HTMLElement>("#status")!;
const stats = document.querySelector<HTMLElement>("#stats")!;
const runtime = document.querySelector<HTMLElement>("#runtime")!;
const gpuDot = document.querySelector<HTMLElement>("#gpu-dot")!;
const prompt = document.querySelector<HTMLTextAreaElement>("#prompt")!;
const fileInput = document.querySelector<HTMLInputElement>("#image")!;
const dropTarget = document.querySelector<HTMLElement>("#drop-target")!;

const renderer = new THREE.WebGPURenderer({
  canvas,
  antialias: true,
  powerPreference: "high-performance",
});
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = DEFAULT_WORLD.exposure;
await renderer.init();

const backend = renderer.backend as { isWebGPUBackend?: boolean };
const usingWebGPU = "gpu" in navigator && backend.isWebGPUBackend === true;
runtime.textContent = usingWebGPU ? "WebGPU renderer" : "WebGL 2 fallback";
gpuDot.dataset.active = String(usingWebGPU);

const scene = new THREE.Scene();
scene.background = new THREE.Color(DEFAULT_WORLD.background);
scene.fog = new THREE.FogExp2(DEFAULT_WORLD.fog, DEFAULT_WORLD.fogDensity);

const camera = new THREE.PerspectiveCamera(52, 1, 0.01, 300);
camera.position.set(0, 0.15, 9.5);

const controls = new OrbitControls(camera, canvas);
controls.enableDamping = true;
controls.dampingFactor = 0.055;
controls.rotateSpeed = 0.55;
controls.zoomSpeed = 0.8;
controls.minDistance = 0.7;
controls.maxDistance = 45;

const root = new THREE.Group();
scene.add(root);

const pointMaterial = new THREE.PointsMaterial({
  size: DEFAULT_WORLD.pointSize,
  sizeAttenuation: true,
  vertexColors: true,
  transparent: true,
  opacity: 0.92,
  depthWrite: false,
  blending: THREE.AdditiveBlending,
});
let cloud = new THREE.Points(new THREE.BufferGeometry(), pointMaterial);
root.add(cloud);

const wire = new THREE.GridHelper(36, 72, 0x263140, 0x11161d);
wire.rotation.x = Math.PI / 2;
wire.position.z = -5.5;
wire.material.transparent = true;
wire.material.opacity = 0.22;
root.add(wire);

const key = new THREE.DirectionalLight(0xcfe8ff, 3.5);
key.position.set(2, 4, 6);
scene.add(key);
scene.add(new THREE.AmbientLight(0x486078, 0.35));

let world: WorldIR = { ...DEFAULT_WORLD };
let sourceField: DepthField | undefined;
let analyser: AnalyserNode | undefined;
let audioSamples: Uint8Array<ArrayBuffer> | undefined;
let lastPointerAt = 0;

function proceduralField() {
  const count = 48_000;
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  const c = new THREE.Color();

  for (let i = 0; i < count; i++) {
    const t = i / count;
    const arm = (i % 7) / 7;
    const radius = 0.4 + Math.pow(t, 0.55) * 5.8;
    const angle = t * 68 + arm * Math.PI * 2;
    const j = i * 3;
    positions[j] = Math.cos(angle) * radius + Math.sin(i * 12.9898) * 0.08;
    positions[j + 1] = (t - 0.5) * 6 + Math.sin(angle * 0.7) * 0.3;
    positions[j + 2] = Math.sin(angle) * radius * 0.55;
    c.setHSL(0.56 + arm * 0.08, 0.55, 0.48 + 0.35 * (1 - t));
    colors[j] = c.r;
    colors[j + 1] = c.g;
    colors[j + 2] = c.b;
  }

  setCloud(positions, colors);
  stats.textContent = count.toLocaleString() + " particles · procedural seed";
}

function setCloud(positions: Float32Array, colors: Float32Array) {
  cloud.geometry.dispose();
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
  geometry.computeBoundingSphere();
  cloud.geometry = geometry;
}

function compileDepthField(field: DepthField) {
  const target = 85_000;
  const stride = Math.max(1, Math.ceil(Math.sqrt((field.width * field.height) / target)));
  const columns = Math.ceil(field.width / stride);
  const rows = Math.ceil(field.height / stride);
  const count = columns * rows;
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  const aspect = field.width / field.height;
  const spanY = 6.2;
  const spanX = spanY * aspect;
  let index = 0;

  for (let y = 0; y < field.height; y += stride) {
    for (let x = 0; x < field.width; x += stride) {
      const pixel = y * field.width + x;
      const d = field.depth[pixel];
      const p = index * 3;
      positions[p] = (x / Math.max(1, field.width - 1) - 0.5) * spanX;
      positions[p + 1] = -(y / Math.max(1, field.height - 1) - 0.5) * spanY;
      positions[p + 2] = (0.5 - d) * world.depthScale;

      const rgba = pixel * 4;
      colors[p] = Math.pow(field.rgba[rgba] / 255, 1.8);
      colors[p + 1] = Math.pow(field.rgba[rgba + 1] / 255, 1.8);
      colors[p + 2] = Math.pow(field.rgba[rgba + 2] / 255, 1.8);
      index++;
    }
  }

  setCloud(positions.slice(0, index * 3), colors.slice(0, index * 3));
  stats.textContent =
    index.toLocaleString() +
    " particles · " +
    field.width +
    "×" +
    field.height +
    " depth · " +
    field.backend.toUpperCase();
}

function applyWorld(next: WorldIR) {
  world = next;
  scene.background = new THREE.Color(world.background);
  scene.fog = new THREE.FogExp2(world.fog, world.fogDensity);
  pointMaterial.size = world.pointSize;
  pointMaterial.needsUpdate = true;
  renderer.toneMappingExposure = world.exposure;
  if (sourceField) compileDepthField(sourceField);
  status.textContent = "World program hot-swapped.";
}

async function compileFile(file: File) {
  if (!file.type.startsWith("image/")) {
    status.textContent = "That file is not an image.";
    return;
  }

  try {
    dropTarget.dataset.visible = "false";
    status.textContent = "Starting local perception…";
    sourceField = await inferDepth(file, (message) => (status.textContent = message));
    compileDepthField(sourceField);
    status.textContent = "Reality compiled locally. Prompt it again to mutate the world.";
    camera.position.set(0, 0, 8.5);
    controls.target.set(0, 0, 0);
    controls.update();
  } catch (error) {
    console.error(error);
    status.textContent = error instanceof Error ? error.message : "Depth compilation failed.";
  }
}

async function enableMicrophone() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const audio = new AudioContext();
    const source = audio.createMediaStreamSource(stream);
    analyser = audio.createAnalyser();
    analyser.fftSize = 512;
    analyser.smoothingTimeConstant = 0.82;
    source.connect(analyser);
    audioSamples = new Uint8Array(analyser.frequencyBinCount);
    status.textContent = "Microphone mapped to world energy.";
    document.querySelector<HTMLButtonElement>("#mic")!.textContent = "Microphone live";
  } catch (error) {
    console.error(error);
    status.textContent = "Microphone permission was not granted.";
  }
}

function audioEnergy() {
  if (!analyser || !audioSamples) return 0;
  analyser.getByteFrequencyData(audioSamples);
  let sum = 0;
  const bins = Math.min(48, audioSamples.length);
  for (let i = 0; i < bins; i++) sum += audioSamples[i];
  return sum / bins / 255;
}

document.querySelector<HTMLButtonElement>("#compile")!.onclick = () => {
  const button = document.querySelector<HTMLButtonElement>("#compile")!;
  button.disabled = true;
  void compileWorldIntent(prompt.value, (message) => (status.textContent = message))
    .then(({ world: next, source }) => {
      applyWorld(next);
      status.textContent =
        source === "local-ai"
          ? "World IR compiled by the local model and hot-swapped."
          : "World IR compiled deterministically.";
    })
    .finally(() => {
      button.disabled = false;
    });
};

document.querySelector<HTMLButtonElement>("#mic")!.onclick = () => void enableMicrophone();

document.querySelector<HTMLButtonElement>("#reset")!.onclick = () => {
  camera.position.set(0, 0.15, 9.5);
  controls.target.set(0, 0, 0);
  controls.update();
};

for (const button of document.querySelectorAll<HTMLButtonElement>(".preset")) {
  button.onclick = () => {
    prompt.value = button.dataset.prompt ?? "";
    applyWorld(compileWorldPrompt(prompt.value));
  };
}

fileInput.onchange = () => {
  const file = fileInput.files?.[0];
  if (file) void compileFile(file);
};

for (const event of ["dragenter", "dragover"]) {
  window.addEventListener(event, (e) => {
    e.preventDefault();
    dropTarget.dataset.visible = "true";
  });
}
window.addEventListener("dragleave", (e) => {
  if (e.target === document.documentElement) dropTarget.dataset.visible = "false";
});
window.addEventListener("drop", (e) => {
  e.preventDefault();
  dropTarget.dataset.visible = "false";
  const file = e.dataTransfer?.files?.[0];
  if (file) void compileFile(file);
});
canvas.addEventListener("pointerdown", () => (lastPointerAt = performance.now()));

function resize() {
  const width = window.innerWidth;
  const height = window.innerHeight;
  renderer.setSize(width, height, false);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
}
window.addEventListener("resize", resize);
resize();

proceduralField();
applyWorld(compileWorldPrompt(prompt.value));

const clock = new THREE.Clock();
renderer.setAnimationLoop(() => {
  const t = clock.getElapsedTime();
  const energy = audioEnergy() * world.audioGain;
  const idle = performance.now() - lastPointerAt > 3500;

  root.rotation.y += world.spin * 0.0025 * (1 + energy * 5);
  root.rotation.z = Math.sin(t * 0.19) * world.drift * 0.08;
  cloud.position.z = Math.sin(t * 0.7) * world.pulse * (0.7 + energy * 4);
  const scale = 1 + energy * 0.075;
  cloud.scale.setScalar(scale);

  if (idle) camera.position.x += Math.sin(t * 0.17) * 0.00055;
  controls.update();
  renderer.render(scene, camera);
});
