# 003 — Events for notification, commands for request-response; bus is namespace-scoped

Status: accepted

## Context
With many extensions, a flat broadcast bus means everyone hears everything and wakes up to discard
messages — noise and latency. Also, "something happened" and "I need an answer now" are different
needs and should not use the same mechanism.

## Decision
Two distinct interaction shapes:
- **Events** — fire-and-forget notifications, many possible listeners. Use for "something happened
  others may care about." Delivery is **namespace-scoped**: the core delivers an event only to
  subscribers whose pattern matches; non-matching extensions are never woken. Event ids are
  `namespace:name@version`.
- **Commands / service requests** — request-response to one named target. Use for "I need a specific
  answer from a specific capability now." Invoked by `namespace:action@version`.

Extensions declare published/subscribed event namespaces in `extension.toml` so the bus can filter
and the bake graph stays statically analyzable.

## Consequences
- 100 extensions are no noisier than 5: no broadcast-then-discard.
- Choosing the wrong shape (e.g. an event where a command was needed) is the main design error to
  watch for in review.
