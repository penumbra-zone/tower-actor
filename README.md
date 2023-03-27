A `tower::Service` backed by an asynchronous actor.

The `tower::Service` trait is very versatile, and allows writing a wide variety
of composable middleware.  However, one gap in existing middleware is bridging
between the request/response-oriented `Service` interface, which is primarily
designed around the idea that requests are processed independently and
asynchronously, and an actor-like model where a stateful actor processes a
queue of incoming messages.

Tower does use an actor model in the form of the `Buffer` middleware for
mediating access to a shared `Service`, by moving the `Service` into a worker
task that pulls request messages from a `mpsc`, and presenting a `Buffer`
handle that wraps the sender side of the `mpsc`.  This is almost what we want,
except that the `Buffer` can only wrap a `Service` implementation, and the
`Service` trait isn't quite what we want: while the `Service` trait's `call`
method allows _synchronous_ mutation of state (via `&mut self`), it doesn't
work well with _asynchronous_ mutation of state (since all asynchronous work
has to be done in the response future, which can't have access to the `Service`
state without additional sharing and synchronization setup).

As a result, in Penumbra's `pd`, we ended up vendoring and forking the `Buffer`
middleware, and using the forked code to wrap hardcoded worker tasks for the
consensus and mempool services we present to `tower-abci`.  To make this more
general, we extracted it into this crate.
