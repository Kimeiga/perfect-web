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
