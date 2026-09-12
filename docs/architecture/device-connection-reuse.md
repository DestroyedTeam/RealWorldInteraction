# Authenticated device connection reuse

The Host MCP transport, JavaScript worker lifetime, device TCP/TLS connection,
native operation lifetime and model turn are distinct. This change concerns only
the host-neutral adapter ↔ device-agent link. No business application, task or
window is a transport prerequisite.

## Negotiation and compatibility

Both peers advertise TLS ALPN `rcu-device-v2-keepalive-v1`. Only a connection that
negotiates this token may carry multiple sequential request/reply frames. Without
the token each call uses one connection, as before. Owned loopback tests exercise
both new-client/old-server and old-client/new-server combinations. This is framing
negotiation, not a downgrade of device wire version, authentication or authority.

Device wire remains v2. Full WebPKI verification plus mutual certificate pins,
TLS 1.3, disabled early data/resumption, bounded newline JSON frames, and exact
reply request/device/instance checks remain in force. Each authenticated stream
pins the first reply's agent instance and rejects a later change.

## Ownership and bounds

`Client` clones share an exclusive-checkout pool. No stream is used by two calls
at once and no synchronous mutex spans network I/O. Per paired client/device:

- Work: at most four connections (Prepare, Execute, cached reads, OpenContext).
- Control: two separate connections (Cancel, Operation, Describe, CloseContext).
- A five-second capacity wait expires with `device_pool_busy_not_sent`.
- Server: existing 32-connection bound; each connection's frame loop is sequential.
- Server idle deadline: 30 seconds; client discards entries idle for 15 seconds.
- Existing five-second connect/first-frame, 90-second call and ten-second reply
  write deadlines remain. Requests are bounded before connection checkout.

Reserved client control capacity prevents its long Executes from holding the
only cancel/query stream. It does not guarantee availability against exhausted
global server capacity, a blocked native provider or another client. Server idle
expiry bounds abandoned authenticated peers; it is not a context/lease expiry.

## Uncertainty, not transparent retry

The call future owns its checked-out stream. Cancellation, timeout, EOF, malformed
reply, write/read failure or identity mismatch drops it. Only a validated reply
returns a negotiated stream to the pool. Domain-error replies are still valid
framed replies, not evidence of a successful native effect.

An idle peer may close unexpectedly. The next call can fail; it is **not retried**,
including when a test peer happens not to have executed it. A caller queries the
original service operation/durable request rather than repeating input. A later
distinct call may open a new authenticated stream. TCP disconnection is not proof
that accepted native execution ended or its resources were released.

## Evidence and diagnostics

`computer.targets` adds one adapter-local `adapter_transport` counter set per
device: authenticated connections, negotiated keepalive connections, reused
checkouts, writes started and replies validated. Other tool results do not repeat
these counters. Counters are shared by client clones, reset with the adapter and
are independently sampled; a concurrent snapshot is not an atomic transaction.
They are not OS action counts, effect verification, latency or model thinking time.

Offline tests cover sequential clone reuse, bounded concurrent exclusivity,
saturated-work cancellation, dropped futures, idle expiry, reply ID/instance
failure, one-shot interoperability, lost replies and peer closure after successful
reuse negotiation. These are owned loopback mTLS tests, not native input or a
Paint/Computer Use end-to-end benchmark. Live evidence is recorded separately.
