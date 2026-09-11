#!/usr/bin/env python3
"""Optional real-browser instrumentation probe; requires Python Playwright.

This exercises the actual two spike pages. It does not run Pleris renderer gates
or establish absence of forced layout. No browser binary is downloaded.
"""
import argparse
import functools
import hashlib
import json
import platform
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from playwright.sync_api import sync_playwright


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser-executable", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--inline", action="store_true", help="load local fixture code in memory without network access")
    parser.add_argument("--no-sandbox", action="store_true", help="only for an isolated local fixture container")
    args = parser.parse_args()
    public = Path(__file__).resolve().parents[1] / "public"
    handler = functools.partial(SimpleHTTPRequestHandler, directory=str(public))
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    result = {"environment": platform.platform(), "kind": "instrumentation smoke test, not performance comparison",
        "delivery": "in-memory module and simulated query string" if args.inline else "local HTTP",
        "module_sha256": hashlib.sha256((public / "loaf.js").read_bytes()).hexdigest(), "modes": {}}
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(executable_path=args.browser_executable,
                headless=True, args=["--no-sandbox"] if args.no_sandbox else [])
            result["browser"] = browser.version
            for mode in ("thrash", "phased"):
                page = browser.new_page(viewport={"width": 1280, "height": 900})
                if args.inline:
                    module = (public / "loaf.js").read_text()
                    page.add_script_tag(type="module", content=module + "\nwindow.__attribution = {installLayoutObserver, summarizeLayoutAttribution};")
                    page.wait_for_function("!!window.__attribution")
                    source = (public / f"{mode}.html").read_text()
                    source = source.replace('import { installLayoutObserver } from "./loaf.js";',
                        'const { installLayoutObserver } = window.__attribution;')
                    source = source.replace("new URLSearchParams(location.search)", 'new URLSearchParams("?n=1200")')
                    page.set_content(source)
                else:
                    page.goto(f"http://127.0.0.1:{server.server_port}/{mode}.html?n=1200")
                page.wait_for_function("typeof window.__run === 'function'")
                sample = page.evaluate("""async () => {
                    const { summarizeLayoutAttribution } = window.__attribution ?? await import('./loaf.js');
                    const raw = [];
                    const observer = new PerformanceObserver(list => {
                      for (const e of list.getEntries()) raw.push({
                        framePropertyType: typeof e.forcedStyleAndLayoutDuration,
                        scriptDurations: e.scripts.map(s => s.forcedStyleAndLayoutDuration)
                      });
                    });
                    observer.observe({type:'long-animation-frame'});
                    window.__loaf.length = 0;
                    const runs = [];
                    for (let i = 0; i < 3; i++) {
                      runs.push(await new Promise(resolve => requestAnimationFrame(() => resolve(window.__run()))));
                    }
                    await new Promise(resolve => setTimeout(resolve, 300));
                    observer.disconnect();
                    return {runs, observation: window.__loafObservation,
                      attribution: summarizeLayoutAttribution(window.__loaf), raw};
                }""")
                result["modes"][mode] = sample
                page.close()
            browser.close()
    finally:
        server.shutdown()
        server.server_close()
        worker.join()
    thrash = result["modes"]["thrash"]
    passed = (thrash["observation"] == "observing"
        and thrash["attribution"]["reportedScripts"] > 0
        and thrash["attribution"]["reportedForcedStyleAndLayoutDuration"] > 0
        and all(row["framePropertyType"] == "undefined" for row in thrash["raw"]))
    result["positive_control_passed"] = passed
    result["limits"] = ["One installed Chromium build on one container",
        "In-memory delivery does not test HTTP loading when --inline is selected",
        "Not the historical Chrome 150 build; not Safari, Firefox, or a real phone",
        "No LoAF records is unobserved attribution, not a zero-layout proof",
        "Wall-clock values are raw smoke-test context, not speedup claims"]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"browser": result["browser"], "positive_control_passed": passed,
        "thrash": thrash["attribution"], "phased": result["modes"]["phased"]["attribution"]}, indent=2))
    if not passed:
        raise SystemExit("Browser did not validate the positive attribution control")


if __name__ == "__main__":
    main()
