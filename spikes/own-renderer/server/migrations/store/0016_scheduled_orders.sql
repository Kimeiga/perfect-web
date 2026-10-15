-- Track `scheduled-orders` (W9): an order placed for a time its reader
-- chooses. The time chosen at checkout, none for as soon as possible, kept
-- with the reader's other choices; and an owner's order's time: the
-- reference of the order it is for, the start of its window and when it is
-- sent to its stores (milliseconds since 1970), and the window's weekday and
-- minute of the day in its first store's zone, which its page words. One for
-- an owner's latest order, as the payment it awaits is (`order_payments`);
-- gone once the order is sent. The in-memory layer keeps the same
-- (`store.rs`' `choices` and `times`).

ALTER TABLE checkout_choices ADD COLUMN scheduled_for bigint;

CREATE TABLE order_times (
    owner     text   PRIMARY KEY,
    reference text   NOT NULL,
    at_ms     bigint NOT NULL,
    sends_ms  bigint NOT NULL CHECK (sends_ms <= at_ms),
    weekday   text   NOT NULL CHECK (weekday IN ('monday', 'tuesday', 'wednesday', 'thursday',
                                                 'friday', 'saturday', 'sunday')),
    minute    bigint NOT NULL CHECK (minute >= 0 AND minute < 1440)
);
