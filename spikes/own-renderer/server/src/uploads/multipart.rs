//! **A form's body, as a browser sends a file in it** (track `uploads`):
//! `multipart/form-data`, RFC 7578, whose parts are delimited as RFC 2046
//! §5.1.1 says.
//!
//! - The boundary is the media type's `boundary` parameter, quoted or not,
//!   1 to 70 characters (RFC 2046 §5.1.1, `boundary := 0*69<bchars>
//!   bcharsnospace`).
//! - "The boundary delimiter MUST occur at the beginning of a line", so a
//!   delimiter is CRLF, `--` and the boundary; the first may begin the body,
//!   after a preamble that is ignored, and the last is followed by `--`.
//! - Each part has a `Content-Disposition: form-data` header with a `name`
//!   (RFC 7578 §4.2). Its `filename` and its `Content-Type` are the
//!   sender's to choose, so neither is read: what the bytes are is sniffed
//!   from them, and what the file is called is never used.

/// One part: its field's name, and its bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct Part<'a> {
    pub name: String,
    pub data: &'a [u8],
}

/// **The boundary a `Content-Type` names**, where it is
/// `multipart/form-data` with one that RFC 2046 allows.
pub fn boundary(content_type: &str) -> Option<String> {
    let mut params = content_type.split(';');
    let media = params.next()?.trim();
    if !media.eq_ignore_ascii_case("multipart/form-data") {
        return None;
    }
    let value = params.find_map(|p| {
        let (k, v) = p.split_once('=')?;
        k.trim().eq_ignore_ascii_case("boundary").then(|| v.trim())
    })?;
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let allowed = |c: char| c.is_ascii_alphanumeric() || "'()+_,-./:=? ".contains(c);
    (!value.is_empty() && value.len() <= 70 && value.chars().all(allowed) && !value.ends_with(' '))
        .then(|| value.to_string())
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

/// **The parts of `body`**, delimited by `boundary`; an error where the body
/// is not delimited as RFC 2046 says, or a part has no field name.
pub fn parts<'a>(body: &'a [u8], boundary: &str) -> Result<Vec<Part<'a>>, &'static str> {
    let dash = format!("--{boundary}");
    let delimiter = format!("\r\n--{boundary}");
    // The first delimiter begins the body, or a line after the preamble.
    let mut at = if body.starts_with(dash.as_bytes()) {
        dash.len()
    } else {
        find(body, delimiter.as_bytes(), 0).ok_or("no boundary delimiter")? + delimiter.len()
    };
    let mut out = Vec::new();
    loop {
        if body.get(at..at + 2) == Some(b"--") {
            return Ok(out);
        }
        // Transport padding, then CRLF.
        while matches!(body.get(at), Some(b' ' | b'\t')) {
            at += 1;
        }
        if body.get(at..at + 2) != Some(b"\r\n") {
            return Err("a boundary delimiter not followed by CRLF");
        }
        at += 2;
        let head_end = find(body, b"\r\n\r\n", at).ok_or("a part's headers do not end")?;
        let head = std::str::from_utf8(&body[at..head_end]).map_err(|_| "a part's headers are not text")?;
        let name = head
            .split("\r\n")
            .find_map(|line| {
                let (k, v) = line.split_once(':')?;
                k.trim()
                    .eq_ignore_ascii_case("content-disposition")
                    .then(|| field_name(v))?
            })
            .ok_or("a part without a form-data name")?;
        let start = head_end + 4;
        let end = find(body, delimiter.as_bytes(), start).ok_or("a part is not closed by a delimiter")?;
        out.push(Part {
            name,
            data: &body[start..end],
        });
        at = end + delimiter.len();
    }
}

/// The `name` of a `form-data` disposition: a quoted string, as browsers
/// write it (HTML's "multipart/form-data encoding algorithm").
fn field_name(disposition: &str) -> Option<String> {
    let mut params = disposition.split(';');
    if !params.next()?.trim().eq_ignore_ascii_case("form-data") {
        return None;
    }
    params.find_map(|p| {
        let (k, v) = p.split_once('=')?;
        if !k.trim().eq_ignore_ascii_case("name") {
            return None;
        }
        let v = v.trim();
        Some(
            v.strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(v)
                .to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boundary_is_the_media_types_parameter() {
        assert_eq!(
            boundary("multipart/form-data; boundary=----WebKitFormBoundaryAbC"),
            Some("----WebKitFormBoundaryAbC".into())
        );
        assert_eq!(boundary("Multipart/Form-Data;boundary=\"a b\""), Some("a b".into()));
        assert_eq!(boundary("multipart/mixed; boundary=x"), None);
        assert_eq!(boundary("multipart/form-data"), None);
        assert_eq!(boundary("multipart/form-data; boundary="), None);
        assert_eq!(boundary(&format!("multipart/form-data; boundary={}", "x".repeat(71))), None);
        assert_eq!(boundary("multipart/form-data; boundary=a\r\nb"), None);
    }

    #[test]
    fn a_forms_parts_are_its_fields() {
        let body = b"--XyZ\r\n\
            Content-Disposition: form-data; name=\"image\"; filename=\"../../etc/passwd.png\"\r\n\
            Content-Type: text/html\r\n\r\n\
            \x89PNG\r\n--Xy-not-it\r\n\
            --XyZ\r\n\
            Content-Disposition: form-data; name=\"note\"\r\n\r\n\
            hi\r\n\
            --XyZ--\r\n";
        let parts = parts(body, "XyZ").expect("parsed");
        assert_eq!(
            parts,
            vec![
                Part {
                    name: "image".into(),
                    data: b"\x89PNG\r\n--Xy-not-it"
                },
                Part {
                    name: "note".into(),
                    data: b"hi"
                },
            ]
        );
    }

    #[test]
    fn a_preamble_is_ignored_and_a_body_without_its_close_refused() {
        let body = b"preamble\r\n--b\r\nContent-Disposition: form-data; name=a\r\n\r\n1\r\n--b--";
        assert_eq!(parts(body, "b").expect("parsed")[0].data, b"1");
        assert!(parts(b"--b\r\nContent-Disposition: form-data; name=a\r\n\r\n1", "b").is_err());
        assert!(parts(b"--b\r\nContent-Type: image/png\r\n\r\n1\r\n--b--", "b").is_err());
        assert!(parts(b"nothing here", "b").is_err());
        assert!(parts(b"--bX\r\n", "b").is_err());
    }
}
