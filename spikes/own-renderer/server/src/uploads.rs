//! **A request whose body is a file** (track `uploads`, `docs/PARALLEL.md`,
//! ADR-0253).
//!
//! The uploads track owns this module: a typed upload with size and
//! content-type limits, stored through a deployment's blob-storage
//! capability, and served safely. `main.rs` reaches it at one marked seam:
//! `Uploads::claims`, asked for every request before its body is read, and
//! `Uploads::answer` for a request it claims. A command's body is bounded to
//! 64 KiB and read whole; an upload's is bounded by its own limits, and read
//! here.
//!
//! Until the track lands, no request is an upload's.

use std::io::BufRead;
use std::net::TcpStream;

/// What the uploads track keeps for one server.
#[derive(Default)]
pub struct Uploads {}

impl Uploads {
    /// **Does this module answer `method` `route`?** Asked before the body is
    /// read, so the command path never reads an upload's body.
    pub fn claims(&self, _method: &str, _route: &str) -> bool {
        false
    }

    /// **Answer a request this module claimed**: read its body from
    /// `reader`, within the upload's limits, and respond on `stream`.
    pub fn answer(
        &self,
        _route: &str,
        _headers: &str,
        _session: &str,
        _fresh: bool,
        _reader: &mut dyn BufRead,
        _stream: &mut TcpStream,
    ) {
    }
}
