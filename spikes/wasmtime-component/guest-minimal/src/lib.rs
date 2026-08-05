//! spike: wasmtime-component — the `no_std` guest.
//!
//! Identical behavior to `../guest`, but without the Rust standard library.
//!
//! Why this exists: the `std` guest imports **15** WASI instances even though its
//! WIT world declares **one** capability. Those 14 extra imports come from Rust's
//! std runtime, not from the component model or from anything the application
//! asked for. This crate measures how much of that surface is avoidable.
//!
//! Charter §14 M8 task 4 requires denying "ambient filesystem, process, network,
//! environment, and secret access by default". An import the guest never
//! requested is exactly such ambient authority, so its size matters.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;

wit_bindgen::generate!({
    path: "../wit",
    world: "public-store-query",
});

use perfect_web::store::stores;

struct Component;

impl Guest for Component {
    fn lookup(id: String) -> String {
        match stores::read(&id) {
            Some(store) => format!("found:{}:{}", store.id, store.name),
            None => format!("missing:{id}"),
        }
    }
}

export!(Component);

// --- minimal runtime support ------------------------------------------------
//
// A bump allocator with no free. Adequate because a component instance handles
// one short call and is then dropped; the whole linear memory goes with it.
// A real Milestone 8 component would use a proper allocator — this is here to
// keep the dependency count at zero so the import list is unambiguous.

const HEAP_SIZE: usize = 64 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut OFFSET: usize = 0;

struct BumpAllocator;

unsafe impl core::alloc::GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe {
            let base = &raw mut HEAP as *mut u8;
            let start = (OFFSET + layout.align() - 1) & !(layout.align() - 1);
            let end = start.saturating_add(layout.size());
            if end > HEAP_SIZE {
                return core::ptr::null_mut();
            }
            OFFSET = end;
            base.add(start)
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // Intentionally a no-op. See the note above.
    }
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// The Canonical ABI requires the guest to export `cabi_realloc` so the host can
/// allocate inside the guest's linear memory when lowering strings and lists.
/// `wit-bindgen`'s `realloc` feature supplies one, but that implementation pulls
/// in `std`; a `no_std` guest must provide its own.
///
/// See <https://github.com/WebAssembly/component-model> — Canonical ABI, `realloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_realloc(
    old_ptr: *mut u8,
    old_len: usize,
    align: usize,
    new_len: usize,
) -> *mut u8 {
    use core::alloc::{GlobalAlloc, Layout};
    unsafe {
        if new_len == 0 {
            return align as *mut u8; // dangling-but-aligned, per the ABI
        }
        let new_layout = Layout::from_size_align_unchecked(new_len, align);
        let new_ptr = ALLOCATOR.alloc(new_layout);
        if !new_ptr.is_null() && !old_ptr.is_null() && old_len > 0 {
            core::ptr::copy_nonoverlapping(old_ptr, new_ptr, core::cmp::min(old_len, new_len));
        }
        new_ptr
    }
}
