-- NEXT 6c, step seven (the integrator's): the payment an owner's order
-- awaits. Its reference, which the payment carries to the provider and back;
-- the provider's payment, once begun; and the provider's page for it, which
-- the order page frames. One for an owner's latest order, as an order is
-- read; gone when the order is placed. The in-memory layer keeps the same
-- (`store.rs`' `awaiting`).

CREATE TABLE order_payments (
    owner     text PRIMARY KEY,
    reference text NOT NULL,
    payment   text NOT NULL DEFAULT '',
    page      text NOT NULL DEFAULT ''
);
