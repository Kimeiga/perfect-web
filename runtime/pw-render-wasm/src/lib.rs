//! **The renderer, compiled for the browser** (ADR-0130).
//!
//! A part a signal decides is rendered again in the browser when the signal
//! changes. This is `runtime/pw-render`, the server's renderer, built for
//! `wasm32-unknown-unknown`, so the browser and the server render a part with
//! one implementation: the same escaping in each of the five contexts, the
//! same anchors, the same refusals. `pw-resume-wasm` exists for the same
//! reason: a second implementation of a security-relevant function is two
//! things that can disagree, and the disagreement is silent.
//!
//! # The ABI
//!
//! ```text
//! alloc(len)              -> ptr     the caller writes a request here
//! render_part(ptr, len)   -> code    0 rendered, 1 refused, 2 malformed
//! out_ptr() / out_len()              the HTML, or why it was refused
//! ```
//!
//! A request is JSON: `{ "part": <a template part>, "values": { name: value } }`,
//! each value in the wire form a page's signals are held in.

use std::sync::Mutex;

static OUT: Mutex<String> = Mutex::new(String::new());

/// Bytes the caller may write a request into. Leaked deliberately, as
/// `pw-resume-wasm` leaks its manifests: a page renders a handful of parts
/// per press, and a free() across the ABI is more ways to be wrong.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = vec![0u8; len];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// **Render one part** with the values given.
///
/// # Safety
///
/// `ptr` must point at `len` bytes the caller obtained from [`alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_part(ptr: *const u8, len: usize) -> u32 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let (code, out) = render(bytes);
    *OUT.lock().unwrap() = out;
    code
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const u8 {
    OUT.lock().unwrap().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn out_len() -> usize {
    OUT.lock().unwrap().len()
}

/// The request's part, rendered: `(0, html)`, `(1, why)` refused by the
/// renderer, or `(2, why)` for a request that is not one.
pub fn render(request: &[u8]) -> (u32, String) {
    let Ok(request) = serde_json::from_slice::<serde_json::Value>(request) else {
        return (2, "the request is not JSON".to_string());
    };
    let part: pw_render::ir::Part = match serde_json::from_value(request["part"].clone()) {
        Ok(p) => p,
        Err(e) => return (2, format!("the part: {e}")),
    };
    let mut env = pw_render::Env::new();
    if let Some(values) = request["values"].as_object() {
        for (name, value) in values {
            env = env.set(name, pw_render::Value::from_wire(value));
        }
    }
    let template = pw_render::Template {
        path: String::new(),
        name: String::new(),
        params: Vec::new(),
        schema: String::new(),
        chunks: vec![pw_render::ir::Chunk::Dynamic(part)],
    };
    match pw_render::render(&template, &env, &[]) {
        Ok(html) => (0, html),
        Err(why) => (1, format!("{why:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::render;

    /// The browser's renderer escapes as the server's does, because it is
    /// the server's: a text part holding markup is text.
    #[test]
    fn a_text_part_is_escaped_as_the_server_escapes_it() {
        let request = br#"{
            "part": { "part": "text", "id": 3, "value": "greeting", "context": "text" },
            "values": { "greeting": "<b>hi</b> & bye" }
        }"#;
        assert_eq!(
            render(request),
            (
                0,
                "<!--pw:s3-->&lt;b&gt;hi&lt;/b&gt; &amp; bye<!--pw:e3-->".to_string()
            )
        );
    }

    /// A `{#match}` renders the arm its signal's case names.
    #[test]
    fn a_match_renders_the_case_it_is_given() {
        let request = br#"{
            "part": { "part": "match", "id": 5, "value": "panel", "arms": [
                { "case": "shut", "body": [{ "chunk": "static", "value": "<p>none</p>" }] },
                { "case": "cart", "body": [{ "chunk": "static", "value": "<aside>cart</aside>" }] }
            ] },
            "values": { "panel": { "$case": "cart" } }
        }"#;
        assert_eq!(
            render(request),
            (0, "<!--pw:s5--><aside>cart</aside><!--pw:e5-->".to_string())
        );
    }

    #[test]
    fn a_request_that_is_not_one_is_refused() {
        assert_eq!(render(b"not json").0, 2);
        assert_eq!(render(br#"{ "part": 1, "values": {} }"#).0, 2);
    }
}
