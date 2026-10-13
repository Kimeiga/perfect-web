//! **A deployment's payments** (NEXT item 6, step 6c): what a payment
//! provider tells the host, held to its signature.
//!
//! A provider tells the host a payment's outcome by a webhook: a request
//! whose body is the event, and whose `Stripe-Signature` header is
//! `t=<seconds>,v1=<hex>`, the HMAC-SHA256 of `"<seconds>.<body>"` under the
//! endpoint's secret (Stripe's scheme, docs.stripe.com "Webhooks", read
//! 2026-10-12). The host acts on an event only once it is shown to be the
//! provider's: signed with the secret, within five minutes of the host's
//! clock, compared in constant time. Fulfilment is never the browser's word.
//!
//! HMAC is written here over `sha2`, which the workspace has, rather than
//! taken as a new crate: it is RFC 2104's two hashes, and RFC 4231's test
//! vectors hold it.

use sha2::{Digest, Sha256};

/// SHA-256's block, in bytes (RFC 2104's `B`).
const BLOCK: usize = 64;

/// **HMAC-SHA256 of `message` under `key`** (RFC 2104): `H((K ^ opad) ||
/// H((K ^ ipad) || message))`, a key longer than a block hashed first.
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether two byte strings are equal, in time that depends on their
/// lengths alone: a signature is never compared byte by byte until the first
/// difference, which would tell an attacker how much of a guess was right.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// **A webhook's `Stripe-Signature` header**, as a provider writes it for
/// `body` at `seconds` under `secret`.
pub(crate) fn signature(secret: &[u8], seconds: i64, body: &[u8]) -> String {
    let mut signed = format!("{seconds}.").into_bytes();
    signed.extend_from_slice(body);
    format!("t={seconds},v1={}", hex(&hmac_sha256(secret, &signed)))
}

/// How far a webhook's time may be from the host's, in seconds: five
/// minutes, Stripe's libraries' default.
pub(crate) const TOLERANCE: i64 = 300;

/// **Why a webhook is not the provider's word.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refused {
    /// No header, or one without a time or a `v1` signature.
    Unsigned,
    /// Signed at a time too far from the host's.
    Stale { seconds: i64, now: i64 },
    /// No `v1` signature is the body's under the secret.
    Forged,
}

/// **Whether `header` signs `body` under `secret`**, at `now` (seconds), as
/// a provider signs it. Every `v1` signature the header carries is tried, as
/// a provider rolling its secret sends two; any other scheme is ignored.
pub(crate) fn verify(
    header: Option<&str>,
    body: &[u8],
    secret: &[u8],
    now: i64,
) -> Result<(), Refused> {
    let header = header.ok_or(Refused::Unsigned)?;
    let mut seconds = None;
    let mut signatures = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", t)) => seconds = t.parse::<i64>().ok(),
            Some(("v1", s)) => signatures.push(s.to_string()),
            _ => {}
        }
    }
    let seconds = seconds.ok_or(Refused::Unsigned)?;
    if signatures.is_empty() {
        return Err(Refused::Unsigned);
    }
    if (now - seconds).abs() > TOLERANCE {
        return Err(Refused::Stale { seconds, now });
    }
    let mut signed = format!("{seconds}.").into_bytes();
    signed.extend_from_slice(body);
    let expected = hex(&hmac_sha256(secret, &signed));
    if signatures
        .iter()
        .any(|s| same(s.as_bytes(), expected.as_bytes()))
    {
        Ok(())
    } else {
        Err(Refused::Forged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    /// **RFC 4231's test cases for HMAC-SHA256**: a short key, a key shorter
    /// than the output, a key of 0xaa bytes, and a key longer than a block.
    #[test]
    fn hmac_sha256_is_rfc_4231s() {
        for (key, data, want) in [
            (
                unhex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b"),
                b"Hi There".to_vec(),
                "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
            ),
            (
                b"Jefe".to_vec(),
                b"what do ya want for nothing?".to_vec(),
                "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
            ),
            (
                vec![0xaa; 20],
                vec![0xdd; 50],
                "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe",
            ),
            (
                vec![0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
                "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
            ),
        ] {
            assert_eq!(hex(&hmac_sha256(&key, &data)), want);
        }
    }

    /// **A webhook signed with the secret, now, is the provider's**; the same
    /// body under another secret, changed, too old or unsigned is not.
    #[test]
    fn a_webhook_is_the_providers_only_as_it_signed_it() {
        let secret = b"whsec_development";
        let body = br#"{"id":"evt_1","type":"payment_intent.succeeded"}"#;
        let header = signature(secret, 1_000_000, body);
        assert_eq!(verify(Some(&header), body, secret, 1_000_000), Ok(()));
        // Within five minutes either way.
        assert_eq!(verify(Some(&header), body, secret, 1_000_300), Ok(()));
        assert_eq!(verify(Some(&header), body, secret, 999_700), Ok(()));
        assert_eq!(
            verify(Some(&header), body, secret, 1_000_301),
            Err(Refused::Stale {
                seconds: 1_000_000,
                now: 1_000_301
            })
        );
        assert_eq!(
            verify(Some(&header), body, b"whsec_other", 1_000_000),
            Err(Refused::Forged)
        );
        let changed = br#"{"id":"evt_1","type":"payment_intent.canceled"}"#;
        assert_eq!(
            verify(Some(&header), changed, secret, 1_000_000),
            Err(Refused::Forged)
        );
        assert_eq!(
            verify(None, body, secret, 1_000_000),
            Err(Refused::Unsigned)
        );
        assert_eq!(
            verify(Some("t=1000000"), body, secret, 1_000_000),
            Err(Refused::Unsigned)
        );
        // A second `v1`, as a provider rolling its secret sends, and an
        // unknown scheme beside it, ignored.
        let rolled = format!("{header},v0=abc,v1={}", "0".repeat(64));
        assert_eq!(verify(Some(&rolled), body, secret, 1_000_000), Ok(()));
        let other_first = format!("t=1000000,v1={},{}", "0".repeat(64), &header[10..]);
        assert_eq!(verify(Some(&other_first), body, secret, 1_000_000), Ok(()));
    }

    /// **Equal lengths compared whole**: a difference anywhere is a
    /// difference, and lengths that differ are never equal.
    #[test]
    fn signatures_are_compared_whole() {
        assert!(same(b"abcd", b"abcd"));
        assert!(!same(b"abcd", b"abce"));
        assert!(!same(b"abcd", b"xbcd"));
        assert!(!same(b"abcd", b"abc"));
        assert!(same(b"", b""));
    }
}

/// **Where a payment is** (Stripe's PaymentIntent lifecycle, docs.stripe.com
/// "How PaymentIntents work", read 2026-10-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    /// Waiting for a card, and where a declined card leaves it, to try
    /// another.
    RequiresPaymentMethod,
    /// The card's bank asks the reader to authenticate (3-D Secure).
    RequiresAction,
    Processing,
    Succeeded,
    Canceled,
}

impl Status {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Status::RequiresPaymentMethod => "requires_payment_method",
            Status::RequiresAction => "requires_action",
            Status::Processing => "processing",
            Status::Succeeded => "succeeded",
            Status::Canceled => "canceled",
        }
    }
}

/// **A payment for one order**, as the provider holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Intent {
    pub(crate) id: String,
    /// What the browser's hosted fields are opened with, and nothing else
    /// may do with it: it names the intent to the provider's own page.
    pub(crate) client_secret: String,
    pub(crate) order: String,
    /// In cents: an amount's minor units (ADR-XXXX).
    pub(crate) amount: i64,
    pub(crate) status: Status,
    /// Why the last card was declined, where one was.
    pub(crate) declined: Option<&'static str>,
}

/// **What the provider tells the host**, each an event it signs and sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Event {
    pub(crate) id: String,
    /// `payment_intent.succeeded`, `payment_intent.payment_failed` or
    /// `payment_intent.requires_action`.
    pub(crate) kind: &'static str,
    pub(crate) intent: String,
}

/// **How a card the hosted fields were given answers**, by Stripe's test
/// numbers (docs.stripe.com "Testing", read 2026-10-12). Any other number is
/// declined: the development provider charges no real card.
fn outcome(card: &str) -> Result<Status, &'static str> {
    let digits: String = card.chars().filter(char::is_ascii_digit).collect();
    match digits.as_str() {
        "4242424242424242" => Ok(Status::Succeeded),
        "4000000000003220" | "4000002760003184" => Ok(Status::RequiresAction),
        "4000000000009995" => Err("insufficient_funds"),
        _ => Err("card_declined"),
    }
}

/// How long an idempotency key answers, in seconds: a day, as Stripe keeps
/// one at least (docs.stripe.com "Idempotent requests", read 2026-10-12).
pub(crate) const KEY_LIFE: i64 = 24 * 60 * 60;

/// **Why the provider refused a request.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderError {
    /// A key sent again with other parameters: Stripe refuses it, and so
    /// does this.
    KeyReused,
    UnknownIntent,
    /// A card given to an intent that takes none now.
    NotPayable(Status),
}

/// **The development payment provider** (NEXT 6c): what a provider such as
/// Stripe does, for a host to be built and tested against, charging nothing.
/// Its intents are idempotent by key; its hosted card fields answer by
/// Stripe's test numbers; each change is an event the host is told of.
#[derive(Debug, Default)]
pub(crate) struct DevProvider {
    intents: std::collections::BTreeMap<String, Intent>,
    /// An idempotency key: what it was first sent with, the intent it made,
    /// and when.
    keys: std::collections::BTreeMap<String, (String, String, i64)>,
    /// Events not yet delivered, in the order they happened.
    pub(crate) outbox: Vec<Event>,
    next: u64,
}

impl DevProvider {
    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}_dev{:06}", self.next)
    }

    /// **An intent for `order`'s `amount`, by `key`** (Stripe's
    /// `Idempotency-Key`): the key sent again within a day with the same
    /// order and amount answers the intent it made, and with others is
    /// refused; a key older than a day makes a new one.
    pub(crate) fn create(
        &mut self,
        key: &str,
        order: &str,
        amount: i64,
        now: i64,
    ) -> Result<Intent, ProviderError> {
        let sent = format!("{order}\u{1f}{amount}");
        if let Some((first, intent, at)) = self.keys.get(key)
            && now - at < KEY_LIFE
        {
            if *first != sent {
                return Err(ProviderError::KeyReused);
            }
            return self
                .intents
                .get(intent)
                .cloned()
                .ok_or(ProviderError::UnknownIntent);
        }
        let id = self.id("pi");
        let intent = Intent {
            client_secret: format!("{id}_secret_{:06}", self.next * 7919 % 1_000_000),
            id: id.clone(),
            order: order.to_string(),
            amount,
            status: Status::RequiresPaymentMethod,
            declined: None,
        };
        self.intents.insert(id.clone(), intent.clone());
        self.keys.insert(key.to_string(), (sent, id, now));
        Ok(intent)
    }

    pub(crate) fn read(&self, id: &str) -> Result<Intent, ProviderError> {
        self.intents
            .get(id)
            .cloned()
            .ok_or(ProviderError::UnknownIntent)
    }

    fn by_secret(&mut self, client_secret: &str) -> Result<&mut Intent, ProviderError> {
        self.intents
            .values_mut()
            .find(|i| i.client_secret == client_secret)
            .ok_or(ProviderError::UnknownIntent)
    }

    fn emit(&mut self, kind: &'static str, intent: String) {
        let id = self.id("evt");
        self.outbox.push(Event { id, kind, intent });
    }

    /// **A card given to the hosted fields of the intent `client_secret`
    /// names**: it succeeds, asks the reader to authenticate, or is declined
    /// and the intent waits for another card.
    pub(crate) fn pay(&mut self, client_secret: &str, card: &str) -> Result<Intent, ProviderError> {
        let intent = self.by_secret(client_secret)?;
        if intent.status != Status::RequiresPaymentMethod {
            return Err(ProviderError::NotPayable(intent.status));
        }
        let (kind, id) = match outcome(card) {
            Ok(status) => {
                intent.status = status;
                intent.declined = None;
                let kind = match status {
                    Status::Succeeded => "payment_intent.succeeded",
                    _ => "payment_intent.requires_action",
                };
                (kind, intent.id.clone())
            }
            Err(why) => {
                intent.declined = Some(why);
                ("payment_intent.payment_failed", intent.id.clone())
            }
        };
        self.emit(kind, id.clone());
        self.read(&id)
    }

    /// **The reader's answer to the bank's challenge**: authenticated, the
    /// payment succeeds; not, it is declined and waits for another card.
    pub(crate) fn authenticate(
        &mut self,
        client_secret: &str,
        passed: bool,
    ) -> Result<Intent, ProviderError> {
        let intent = self.by_secret(client_secret)?;
        if intent.status != Status::RequiresAction {
            return Err(ProviderError::NotPayable(intent.status));
        }
        let id = intent.id.clone();
        if passed {
            intent.status = Status::Succeeded;
            self.emit("payment_intent.succeeded", id.clone());
        } else {
            intent.status = Status::RequiresPaymentMethod;
            intent.declined = Some("authentication_failed");
            self.emit("payment_intent.payment_failed", id.clone());
        }
        self.read(&id)
    }

    /// **An event as the provider sends it**: its body, and the signature
    /// header for it at `now` under `secret`.
    pub(crate) fn sent(event: &Event, secret: &[u8], now: i64) -> (String, String) {
        let body = serde_json::json!({
            "id": event.id,
            "type": event.kind,
            "data": { "object": { "id": event.intent } },
        })
        .to_string();
        let header = signature(secret, now, body.as_bytes());
        (body, header)
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    /// **One intent per key**: sent again within a day with the same order
    /// and amount, the same intent; with another amount, refused; after a
    /// day, a new one.
    #[test]
    fn an_intent_is_made_once_per_key() {
        let mut p = DevProvider::default();
        let first = p
            .create("order-1:press-a", "order-1", 1250, 0)
            .expect("made");
        assert_eq!(first.status, Status::RequiresPaymentMethod);
        let again = p
            .create("order-1:press-a", "order-1", 1250, 60)
            .expect("answered");
        assert_eq!(again, first);
        assert_eq!(
            p.create("order-1:press-a", "order-1", 1300, 60),
            Err(ProviderError::KeyReused)
        );
        let later = p
            .create("order-1:press-a", "order-1", 1250, KEY_LIFE)
            .expect("made again");
        assert_ne!(later.id, first.id);
        // Another key is another intent.
        let other = p
            .create("order-2:press-a", "order-2", 1250, 0)
            .expect("made");
        assert_ne!(other.id, first.id);
    }

    /// **A card's outcome by Stripe's test numbers**: 4242 succeeds; 3220
    /// asks to authenticate, and the answer decides; 0002 is declined and
    /// 9995 has insufficient funds, each leaving the intent for another card;
    /// a paid intent takes no card.
    #[test]
    fn a_cards_outcome_is_its_test_numbers() {
        let mut p = DevProvider::default();
        let i = p.create("k1", "order-1", 1250, 0).expect("made");
        let declined = p
            .pay(&i.client_secret, "4000 0000 0000 0002")
            .expect("answered");
        assert_eq!(
            (declined.status, declined.declined),
            (Status::RequiresPaymentMethod, Some("card_declined"))
        );
        let poor = p
            .pay(&i.client_secret, "4000000000009995")
            .expect("answered");
        assert_eq!(poor.declined, Some("insufficient_funds"));
        let paid = p
            .pay(&i.client_secret, "4242 4242 4242 4242")
            .expect("answered");
        assert_eq!((paid.status, paid.declined), (Status::Succeeded, None));
        assert_eq!(
            p.pay(&i.client_secret, "4242424242424242"),
            Err(ProviderError::NotPayable(Status::Succeeded))
        );
        let kinds: Vec<&str> = p.outbox.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                "payment_intent.payment_failed",
                "payment_intent.payment_failed",
                "payment_intent.succeeded"
            ]
        );

        let j = p.create("k2", "order-2", 990, 0).expect("made");
        let asked = p
            .pay(&j.client_secret, "4000000000003220")
            .expect("answered");
        assert_eq!(asked.status, Status::RequiresAction);
        let failed = p.authenticate(&j.client_secret, false).expect("answered");
        assert_eq!(
            (failed.status, failed.declined),
            (Status::RequiresPaymentMethod, Some("authentication_failed"))
        );
        p.pay(&j.client_secret, "4000000000003220")
            .expect("asked again");
        let passed = p.authenticate(&j.client_secret, true).expect("answered");
        assert_eq!(passed.status, Status::Succeeded);
        assert_eq!(
            p.pay("pi_nothing_secret_0", "4242424242424242"),
            Err(ProviderError::UnknownIntent)
        );
    }

    /// **What the provider sends is what the host verifies.**
    #[test]
    fn an_event_sent_is_signed_for_the_host() {
        let mut p = DevProvider::default();
        let i = p.create("k", "order-1", 1250, 0).expect("made");
        p.pay(&i.client_secret, "4242424242424242").expect("paid");
        let (body, header) = DevProvider::sent(&p.outbox[0], b"whsec_dev", 1_000);
        assert_eq!(
            verify(Some(&header), body.as_bytes(), b"whsec_dev", 1_000),
            Ok(())
        );
        let json: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(json["type"], "payment_intent.succeeded");
        assert_eq!(json["data"]["object"]["id"], i.id.as_str());
    }
}

// --- the development provider, on its own origin ---------------------------

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A request as the provider reads one: bounded, as a card number and a
/// few fields are all it takes.
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// A form field of the body (`application/x-www-form-urlencoded`).
    fn field(&self, name: &str) -> Option<String> {
        String::from_utf8_lossy(&self.body)
            .split('&')
            .find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k == name).then(|| v.replace('+', " "))
            })
    }
}

const MAX_BODY: usize = 16 * 1024;

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_string(), parts.next()?.to_string());
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).ok()? == 0 || h == "\r\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return None;
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

fn write(stream: &mut TcpStream, code: u16, mime: &str, extra: &str, body: &[u8]) {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\ncontent-type: {mime}\r\ncontent-length: {}\r\n{extra}cache-control: no-store\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// `1250` as `$12.50`, for the hosted fields' button.
fn dollars(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.unsigned_abs();
    format!("{sign}${}.{:02}", c / 100, c % 100)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn intent_json(i: &Intent) -> String {
    serde_json::json!({
        "id": i.id,
        "client_secret": i.client_secret,
        "status": i.status.name(),
        "amount": i.amount,
        "currency": "usd",
        "metadata": { "order": i.order },
        "last_payment_error": i.declined.map(|code| serde_json::json!({ "code": code })),
    })
    .to_string()
}

/// **The development provider, served on its own origin** (a port of its
/// own): the API a host calls, the hosted card fields a page embeds, and the
/// webhooks it sends the host, each signed.
pub(crate) struct Origin {
    provider: Mutex<DevProvider>,
    secret: Vec<u8>,
    /// Where the host hears the provider: its `/payments/webhook`.
    webhook: String,
    /// The host's origin, the one page that may embed the hosted fields.
    host: String,
    now: fn() -> i64,
    /// A test's: each event sent twice, as a provider may.
    pub(crate) twice: AtomicBool,
    /// A test's: events held, and sent in reverse when let go.
    held: Mutex<Option<Vec<Event>>>,
}

/// The wall clock in seconds since 1970.
pub(crate) fn wall_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Origin {
    pub(crate) fn new(secret: &[u8], webhook: &str, host: &str, now: fn() -> i64) -> Origin {
        Origin {
            provider: Mutex::new(DevProvider::default()),
            secret: secret.to_vec(),
            webhook: webhook.to_string(),
            host: host.to_string(),
            now,
            twice: AtomicBool::new(false),
            held: Mutex::new(None),
        }
    }

    /// A test's: events held from now until [`Origin::release_reversed`].
    pub(crate) fn hold(&self) {
        *self.held.lock().expect("held") = Some(Vec::new());
    }

    /// A test's: the held events sent, the last first.
    pub(crate) fn release_reversed(&self) {
        let held = self.held.lock().expect("held").take().unwrap_or_default();
        for e in held.iter().rev() {
            self.send(e);
        }
    }

    fn send(&self, e: &Event) {
        let (body, header) = DevProvider::sent(e, &self.secret, (self.now)());
        let times = if self.twice.load(Ordering::SeqCst) {
            2
        } else {
            1
        };
        for _ in 0..times {
            if let Err(why) = post(&self.webhook, &body, &header) {
                eprintln!(
                    "development payments: the webhook to {}: {why}",
                    self.webhook
                );
            }
        }
    }

    /// Each event the provider has not yet told the host, told, in order;
    /// or held, where a test holds them.
    fn deliver(&self) {
        let events = std::mem::take(&mut self.provider.lock().expect("provider").outbox);
        let mut held = self.held.lock().expect("held");
        if let Some(h) = held.as_mut() {
            h.extend(events);
            return;
        }
        drop(held);
        for e in &events {
            self.send(e);
        }
    }

    fn page(&self, intent: &Intent, said: &str) -> String {
        let secret = escape(&intent.client_secret);
        let form = match intent.status {
            Status::RequiresPaymentMethod => format!(
                "<form method=\"post\" action=\"/pay/{secret}\"><label>Card number \
                 <input name=\"card\" inputmode=\"numeric\" autocomplete=\"cc-number\" required></label> \
                 <button>Pay {}</button></form>",
                dollars(intent.amount)
            ),
            Status::RequiresAction => format!(
                "<p>Your bank asks you to confirm this payment.</p>\
                 <form method=\"post\" action=\"/pay/{secret}/authenticate\">\
                 <button name=\"passed\" value=\"yes\">Confirm</button> \
                 <button name=\"passed\" value=\"no\">Decline</button></form>"
            ),
            Status::Succeeded => "<p>Paid.</p>".to_string(),
            Status::Processing => "<p>Processing.</p>".to_string(),
            Status::Canceled => "<p>This payment was canceled.</p>".to_string(),
        };
        format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
             <title>Card details</title></head><body><main>{form}\
             <p role=\"status\">{}</p></main></body></html>",
            escape(said)
        )
    }

    fn answer(&self, mut stream: TcpStream) {
        let Some(req) = read_request(&stream) else {
            return;
        };
        let path = req.path.split('?').next().unwrap_or("/").to_string();
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        // The hosted fields: embedded by the host's page alone, and running
        // no script at all.
        let framed = format!(
            "content-security-policy: default-src 'none'; form-action 'self'; frame-ancestors {}\r\n",
            self.host
        );
        let html = "text/html; charset=utf-8";
        let json = "application/json";
        match (req.method.as_str(), segments.as_slice()) {
            ("POST", ["v1", "payment_intents"]) => {
                let Some(key) = req.header("idempotency-key").map(str::to_string) else {
                    return write(&mut stream, 400, json, "", br#"{"error":{"type":"invalid_request_error","message":"an Idempotency-Key is required"}}"#);
                };
                let asked: serde_json::Value =
                    serde_json::from_slice(&req.body).unwrap_or_default();
                let (Some(order), Some(amount)) =
                    (asked["order"].as_str(), asked["amount"].as_i64())
                else {
                    return write(&mut stream, 400, json, "", br#"{"error":{"type":"invalid_request_error","message":"an order and an amount are required"}}"#);
                };
                let made = self.provider.lock().expect("provider").create(
                    &key,
                    order,
                    amount,
                    (self.now)(),
                );
                match made {
                    Ok(i) => write(&mut stream, 200, json, "", intent_json(&i).as_bytes()),
                    Err(_) => write(&mut stream, 400, json, "", br#"{"error":{"type":"idempotency_error","message":"the key was sent with other parameters"}}"#),
                }
            }
            ("GET", ["v1", "payment_intents", id]) => {
                match self.provider.lock().expect("provider").read(id) {
                    Ok(i) => write(&mut stream, 200, json, "", intent_json(&i).as_bytes()),
                    Err(_) => write(
                        &mut stream,
                        404,
                        json,
                        "",
                        br#"{"error":{"type":"invalid_request_error","message":"no such intent"}}"#,
                    ),
                }
            }
            ("GET", ["pay", secret]) => {
                let found = self
                    .provider
                    .lock()
                    .expect("provider")
                    .intents
                    .values()
                    .find(|i| i.client_secret == *secret)
                    .cloned();
                match found {
                    Some(i) => write(
                        &mut stream,
                        200,
                        html,
                        &framed,
                        self.page(&i, "").as_bytes(),
                    ),
                    None => write(
                        &mut stream,
                        404,
                        html,
                        &framed,
                        b"<!doctype html><title>Not found</title>",
                    ),
                }
            }
            ("POST", ["pay", secret]) => {
                let card = req.field("card").unwrap_or_default();
                let paid = self.provider.lock().expect("provider").pay(secret, &card);
                match paid {
                    Ok(i) => {
                        let said = match i.declined {
                            Some("insufficient_funds") => "Your card has insufficient funds.",
                            Some(_) => "Your card was declined.",
                            None => "",
                        };
                        write(
                            &mut stream,
                            200,
                            html,
                            &framed,
                            self.page(&i, said).as_bytes(),
                        );
                        self.deliver();
                    }
                    Err(_) => write(
                        &mut stream,
                        409,
                        html,
                        &framed,
                        b"<!doctype html><title>Not payable</title>",
                    ),
                }
            }
            ("POST", ["pay", secret, "authenticate"]) => {
                let passed = req.field("passed").as_deref() == Some("yes");
                let answered = self
                    .provider
                    .lock()
                    .expect("provider")
                    .authenticate(secret, passed);
                match answered {
                    Ok(i) => {
                        let said = if passed {
                            ""
                        } else {
                            "The payment was not confirmed."
                        };
                        write(
                            &mut stream,
                            200,
                            html,
                            &framed,
                            self.page(&i, said).as_bytes(),
                        );
                        self.deliver();
                    }
                    Err(_) => write(
                        &mut stream,
                        409,
                        html,
                        &framed,
                        b"<!doctype html><title>Not payable</title>",
                    ),
                }
            }
            _ => write(&mut stream, 404, "text/plain", "", b"not found"),
        }
    }
}

/// **The provider's origin, listening**: a thread per connection.
pub(crate) fn serve(listener: TcpListener, origin: Arc<Origin>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let origin = origin.clone();
        std::thread::spawn(move || origin.answer(stream));
    }
}

/// **A POST of `body` to `url`** (`http://host:port/path`), with the
/// provider's signature: the webhook, over a connection of its own.
pub(crate) fn post(url: &str, body: &str, signature: &str) -> Result<u16, String> {
    let rest = url.strip_prefix("http://").ok_or("not an http URL")?;
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_string()), |(a, p)| (a, format!("/{p}")));
    let mut stream = TcpStream::connect(authority).map_err(|e| e.to_string())?;
    let head = format!(
        "POST {path} HTTP/1.1\r\nhost: {authority}\r\ncontent-type: application/json\r\nstripe-signature: {signature}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .map_err(|e| e.to_string())?;
    stream
        .write_all(body.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    answer
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| format!("no status: {answer:?}"))
}

#[cfg(test)]
mod origin_tests {
    use super::*;

    /// Each request a test's webhook endpoint heard: its signature header,
    /// and its body.
    type Heard = Arc<Mutex<Vec<(String, String)>>>;

    /// A host's webhook endpoint, for a test: it answers 200 and keeps each
    /// request's signature header and body.
    fn webhook_listener() -> (String, Heard) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!(
            "http://{}/payments/webhook",
            listener.local_addr().expect("addr")
        );
        let heard = Arc::new(Mutex::new(Vec::new()));
        let kept = heard.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                if let Some(req) = read_request(&stream) {
                    let sig = req.header("stripe-signature").unwrap_or("").to_string();
                    kept.lock()
                        .expect("heard")
                        .push((sig, String::from_utf8_lossy(&req.body).into_owned()));
                }
                write(&mut stream, 200, "text/plain", "", b"ok");
            }
        });
        (url, heard)
    }

    fn started(webhook: &str) -> (String, Arc<Origin>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let origin = Arc::new(Origin::new(
            b"whsec_test",
            webhook,
            "http://127.0.0.1:3301",
            || 1_000_000,
        ));
        let served = origin.clone();
        std::thread::spawn(move || serve(listener, served));
        (base, origin)
    }

    /// A request to the provider, answered: its status and body.
    fn ask(
        base: &str,
        method: &str,
        path: &str,
        headers: &str,
        body: &str,
    ) -> (u16, String, String) {
        let authority = base.strip_prefix("http://").expect("http");
        let mut stream = TcpStream::connect(authority).expect("connect");
        let head = format!(
            "{method} {path} HTTP/1.1\r\nhost: {authority}\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).expect("write");
        stream.write_all(body.as_bytes()).expect("write");
        let mut all = String::new();
        stream.read_to_string(&mut all).expect("read");
        let (head, body) = all.split_once("\r\n\r\n").expect("a head");
        let code = head
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .expect("status");
        (code, head.to_string(), body.to_string())
    }

    /// **The provider's API, its hosted fields and its webhook, over the
    /// wire**: an intent made once per key, a card paid in the hosted fields,
    /// and the host told, signed.
    #[test]
    fn a_card_paid_in_the_hosted_fields_is_told_to_the_host_signed() {
        let (webhook, heard) = webhook_listener();
        let (base, _origin) = started(&webhook);
        let key = "idempotency-key: order-1:press-a\r\n";
        let (code, _, made) = ask(
            &base,
            "POST",
            "/v1/payment_intents",
            key,
            r#"{"order":"order-1","amount":1250}"#,
        );
        assert_eq!(code, 200, "{made}");
        let made: serde_json::Value = serde_json::from_str(&made).expect("json");
        assert_eq!(made["status"], "requires_payment_method");
        let (_, _, again) = ask(
            &base,
            "POST",
            "/v1/payment_intents",
            key,
            r#"{"order":"order-1","amount":1250}"#,
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&again).expect("json")["id"],
            made["id"]
        );
        let (code, _, refused) = ask(
            &base,
            "POST",
            "/v1/payment_intents",
            key,
            r#"{"order":"order-1","amount":9999}"#,
        );
        assert_eq!(code, 400, "{refused}");
        assert!(refused.contains("idempotency_error"));
        let (code, _, _) = ask(
            &base,
            "POST",
            "/v1/payment_intents",
            "",
            r#"{"order":"order-1","amount":1250}"#,
        );
        assert_eq!(code, 400, "a request without a key is refused");

        // The hosted fields: a form, embedded by the host alone, no script.
        let secret = made["client_secret"].as_str().expect("secret");
        let (code, head, page) = ask(&base, "GET", &format!("/pay/{secret}"), "", "");
        assert_eq!(code, 200);
        assert!(page.contains("Pay $12.50"), "{page}");
        assert!(
            head.contains("frame-ancestors http://127.0.0.1:3301"),
            "{head}"
        );
        assert!(head.contains("default-src 'none'"), "{head}");

        // Declined, then paid: two events, signed for the host.
        let form = "content-type: application/x-www-form-urlencoded\r\n";
        let (_, _, declined) = ask(
            &base,
            "POST",
            &format!("/pay/{secret}"),
            form,
            "card=4000+0000+0000+0002",
        );
        assert!(declined.contains("Your card was declined."), "{declined}");
        let (_, _, paid) = ask(
            &base,
            "POST",
            &format!("/pay/{secret}"),
            form,
            "card=4242424242424242",
        );
        assert!(paid.contains("Paid."), "{paid}");
        let heard = heard.lock().expect("heard").clone();
        assert_eq!(heard.len(), 2, "{heard:?}");
        for (sig, body) in &heard {
            assert_eq!(
                verify(Some(sig), body.as_bytes(), b"whsec_test", 1_000_000),
                Ok(())
            );
        }
        assert!(heard[0].1.contains("payment_intent.payment_failed"));
        assert!(heard[1].1.contains("payment_intent.succeeded"));
        let (_, _, read) = ask(
            &base,
            "GET",
            &format!("/v1/payment_intents/{}", made["id"].as_str().expect("id")),
            "",
            "",
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&read).expect("json")["status"],
            "succeeded"
        );
    }

    /// **A provider may send an event twice, and out of order**: the test
    /// switches the host is held to.
    #[test]
    fn the_provider_can_send_twice_and_out_of_order() {
        let (webhook, heard) = webhook_listener();
        let (base, origin) = started(&webhook);
        let (_, _, made) = ask(
            &base,
            "POST",
            "/v1/payment_intents",
            "idempotency-key: k\r\n",
            r#"{"order":"o","amount":500}"#,
        );
        let made: serde_json::Value = serde_json::from_str(&made).expect("json");
        let secret = made["client_secret"].as_str().expect("secret").to_string();
        let form = "content-type: application/x-www-form-urlencoded\r\n";
        origin.hold();
        ask(
            &base,
            "POST",
            &format!("/pay/{secret}"),
            form,
            "card=4000000000000002",
        );
        ask(
            &base,
            "POST",
            &format!("/pay/{secret}"),
            form,
            "card=4242424242424242",
        );
        assert!(heard.lock().expect("heard").is_empty(), "held");
        origin.twice.store(true, Ordering::SeqCst);
        origin.release_reversed();
        let kinds: Vec<String> = heard
            .lock()
            .expect("heard")
            .iter()
            .map(|(_, b)| {
                serde_json::from_str::<serde_json::Value>(b).expect("json")["type"]
                    .as_str()
                    .expect("type")
                    .to_string()
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "payment_intent.succeeded",
                "payment_intent.succeeded",
                "payment_intent.payment_failed",
                "payment_intent.payment_failed"
            ]
        );
    }
}

// --- the host, hearing the provider ---------------------------------------

/// **A GET of `url`** (`http://host:port/path`): its status and body.
pub(crate) fn get(url: &str) -> Result<(u16, String), String> {
    let rest = url.strip_prefix("http://").ok_or("not an http URL")?;
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_string()), |(a, p)| (a, format!("/{p}")));
    let mut stream = TcpStream::connect(authority).map_err(|e| e.to_string())?;
    let head = format!("GET {path} HTTP/1.1\r\nhost: {authority}\r\nconnection: close\r\n\r\n");
    stream
        .write_all(head.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut all = String::new();
    stream.read_to_string(&mut all).map_err(|e| e.to_string())?;
    let (head, body) = all.split_once("\r\n\r\n").ok_or("no head")?;
    let code = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or("no status")?;
    Ok((code, body.to_string()))
}

/// **An intent as the provider says it now**, read again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settled {
    pub(crate) intent: String,
    pub(crate) order: String,
    pub(crate) amount: i64,
    pub(crate) status: String,
}

/// **Why the host did not act on a webhook.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotActed {
    /// Not the provider's word (`verify`).
    Refused(Refused),
    /// The provider's word, heard before: answered, and nothing done again.
    HeardBefore,
    /// Signed, but no event the host reads, or one naming no intent the
    /// provider has.
    Unreadable(String),
}

/// **What the host hears from its provider** (NEXT 6c). An event is acted
/// on only as the provider signed it, and then not as the event says but as
/// the provider says the intent is when asked again: an event may arrive
/// twice, or after a later one, and the intent's state now is what holds.
/// An event heard before is answered and ignored. What the host then does
/// is a transition that holds once (an order awaiting payment placed), so
/// an event heard twice across a restart changes nothing the second time;
/// the ids kept here spare the provider a second read.
pub(crate) struct Hearing {
    secret: Vec<u8>,
    /// The provider's API, where an intent is read again.
    api: String,
    heard: Mutex<std::collections::BTreeSet<String>>,
}

impl Hearing {
    pub(crate) fn new(secret: &[u8], api: &str) -> Hearing {
        Hearing {
            secret: secret.to_vec(),
            api: api.trim_end_matches('/').to_string(),
            heard: Mutex::new(std::collections::BTreeSet::new()),
        }
    }

    /// **A webhook, heard at `now`**: the intent it names, as the provider
    /// says it now, where the host should act on it.
    pub(crate) fn webhook(
        &self,
        header: Option<&str>,
        body: &[u8],
        now: i64,
    ) -> Result<Settled, NotActed> {
        verify(header, body, &self.secret, now).map_err(NotActed::Refused)?;
        let event: serde_json::Value =
            serde_json::from_slice(body).map_err(|e| NotActed::Unreadable(e.to_string()))?;
        let (Some(id), Some(intent)) =
            (event["id"].as_str(), event["data"]["object"]["id"].as_str())
        else {
            return Err(NotActed::Unreadable("no event id or intent".into()));
        };
        if !self.heard.lock().expect("heard").insert(id.to_string()) {
            return Err(NotActed::HeardBefore);
        }
        let (code, read) = get(&format!("{}/v1/payment_intents/{intent}", self.api))
            .map_err(NotActed::Unreadable)?;
        if code != 200 {
            return Err(NotActed::Unreadable(format!(
                "the provider answered {code} for {intent}"
            )));
        }
        let read: serde_json::Value =
            serde_json::from_str(&read).map_err(|e| NotActed::Unreadable(e.to_string()))?;
        Ok(Settled {
            intent: intent.to_string(),
            order: read["metadata"]["order"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            amount: read["amount"].as_i64().unwrap_or_default(),
            status: read["status"].as_str().unwrap_or_default().to_string(),
        })
    }
}

#[cfg(test)]
mod hearing_tests {
    use super::*;

    fn provider() -> (String, Arc<Origin>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        // Its webhooks go nowhere here: each test hands the host the events
        // as the provider would send them.
        let origin = Arc::new(Origin::new(
            b"whsec_t",
            "http://127.0.0.1:9/x",
            "http://127.0.0.1:3301",
            || 5_000,
        ));
        let served = origin.clone();
        std::thread::spawn(move || serve(listener, served));
        (base, origin)
    }

    /// **The host acts on the intent as the provider says it now**: a
    /// signed event is read again; heard twice, ignored the second time;
    /// forged or stale, refused; a failure that arrives after the success
    /// reads as succeeded, which is what the intent is.
    #[test]
    fn the_host_acts_on_the_intent_as_the_provider_says_it_now() {
        let (base, origin) = provider();
        let hearing = Hearing::new(b"whsec_t", &base);
        let id = {
            let mut p = origin.provider.lock().expect("provider");
            let i = p.create("k", "order-7", 1250, 5_000).expect("made");
            p.pay(&i.client_secret, "4000000000000002")
                .expect("declined");
            p.pay(&i.client_secret, "4242424242424242").expect("paid");
            i.id
        };
        let events = std::mem::take(&mut origin.provider.lock().expect("provider").outbox);
        let (failed, succeeded) = (&events[0], &events[1]);

        let (body, header) = DevProvider::sent(succeeded, b"whsec_t", 5_000);
        let settled = hearing
            .webhook(Some(&header), body.as_bytes(), 5_000)
            .expect("acted on");
        assert_eq!(
            settled,
            Settled {
                intent: id.clone(),
                order: "order-7".into(),
                amount: 1250,
                status: "succeeded".into()
            }
        );
        assert_eq!(
            hearing.webhook(Some(&header), body.as_bytes(), 5_010),
            Err(NotActed::HeardBefore)
        );

        // The failure, delivered late: read again, the intent succeeded.
        let (body, header) = DevProvider::sent(failed, b"whsec_t", 5_000);
        let late = hearing
            .webhook(Some(&header), body.as_bytes(), 5_020)
            .expect("acted on");
        assert_eq!(late.status, "succeeded");

        // Not the provider's word.
        let forged = DevProvider::sent(
            &Event {
                id: "evt_x".into(),
                kind: "payment_intent.succeeded",
                intent: id.clone(),
            },
            b"whsec_other",
            5_000,
        );
        assert!(matches!(
            hearing.webhook(Some(&forged.1), forged.0.as_bytes(), 5_000),
            Err(NotActed::Refused(Refused::Forged))
        ));
        let stale = DevProvider::sent(
            &Event {
                id: "evt_y".into(),
                kind: "payment_intent.succeeded",
                intent: id,
            },
            b"whsec_t",
            4_000,
        );
        assert!(matches!(
            hearing.webhook(Some(&stale.1), stale.0.as_bytes(), 5_000),
            Err(NotActed::Refused(Refused::Stale { .. }))
        ));
        let unknown = DevProvider::sent(
            &Event {
                id: "evt_z".into(),
                kind: "payment_intent.succeeded",
                intent: "pi_none".into(),
            },
            b"whsec_t",
            5_000,
        );
        assert!(matches!(
            hearing.webhook(Some(&unknown.1), unknown.0.as_bytes(), 5_000),
            Err(NotActed::Unreadable(_))
        ));
    }
}
