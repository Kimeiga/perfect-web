# Reality Compiler spike

A browser-only experiment for compiling a photograph into an explorable GPU world.

## What is implemented

1. Drop or select any local image.
2. Transformers.js 4.3 loads `onnx-community/depth-anything-v2-small` in the browser.
3. WebGPU inference is attempted first, with the default WASM path as a fallback.
4. The normalized depth field and source pixels become an interactive 3D point cloud.
5. Three.js WebGPURenderer draws the world, with its WebGL 2 fallback when WebGPU rendering is unavailable.
6. On WebGPU browsers, LFM2.5 350M runs locally and Transformers.js 4.3 constrains its output to a strict World IR JSON schema. If that model path is unavailable, the deterministic prompt compiler remains the fallback.
7. Optional microphone energy drives the world without sending audio anywhere.

The source image stays local. Model weights are downloaded from Hugging Face on first use and then handled by the browser cache.

## Run

~~~sh
cd spikes/reality-compiler
npm install
npm run dev
~~~

Build/typecheck:

~~~sh
npm run build
~~~

## Why this spike exists

The useful boundary is **AI interprets; deterministic graphics execute**. The depth model runs when a source changes. It is not in the frame loop. Rendering and interaction remain normal GPU work.

The renderer only accepts a bounded World IR. Local text generation can choose those values, but it cannot inject code or enter the frame loop. Schema-constrained generation and the deterministic fallback therefore share the same renderer contract.

## Next experiments

- TSL compute particles that collide with the reconstructed depth surface.
- Local segmentation and object masks.
- Camera/video input as a continuously updated semantic field.
- Deterministic headless shader renders and visual regression checks with vgpu.
- Export a compiled interaction as a portable World IR + media bundle.
