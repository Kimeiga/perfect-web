# The Web, Recompiled — concept kit

A working communication package for a research project exploring a semantic application platform spanning browser, edge, and origin.

**Working thesis:** describe what the application means; let the platform decide how it runs.

## Files

- `index.html` / `landing.html` — self-contained, responsive one-page launch-site prototype. They are identical; `index.html` is ready for static hosting. It has no external runtime dependencies.
- `proof-roadmap.md` — milestone-by-milestone validation plan, flagship demonstrations, launch narrative, audience messaging, and publication strategy.
- `whitepaper.md` — version 0.1 technical design paper covering the language, effects, resources, privacy, placement, rendering, resumption, caching, runtime, browser, network, AI evaluation, limitations, and adoption.

## Preview locally

Open `index.html` directly, or serve the directory so all relative links behave like a deployed site:

```bash
cd web-recompiled
python3 -m http.server 8080
```

Then visit `http://localhost:8080/`.

## Publish as a static site

The folder can be deployed unchanged to a static host. For a repository-backed launch, use `index.html` as the entry page. Keep the two Markdown documents beside it, or render them into site routes later.

## What the first real release should contain

1. An executable Bug Museum of accepted and rejected programs.
2. A streamed store page with public menu data and a private cart.
3. A reproduction of request amplification caused by generic UI effects, followed by a version that cannot express the failure.
4. A compiler-generated browser–edge–origin architecture graph.
5. A static route that ships zero application JavaScript.
6. A compile-time rejection of private data entering a shared cache.
7. One hostile-network recording with reproducible traces.
8. A versioned white paper and benchmark methodology.

The current name, syntax, and visual identity are intentionally replaceable. The proof structure is the durable part.
