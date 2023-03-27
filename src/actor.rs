use std::{
    future::Future,
    task::{Context, Poll},
};

use crate::future::ActorFuture;
use crate::message::Message;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::PollSender;
use tower::Service;
use tracing::Span;

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("Actor has terminated or panicked")]
    ActorTerminated,
    #[error("Actor has dropped this request without responding")]
    ActorHungUp,
}

pub struct Actor<R, S, E> {
    queue: tokio_util::sync::PollSender<Message<R, S, E>>,
}

impl<R, S, E> Actor<R, S, E>
where
    R: Send + 'static,
    S: Send + 'static,
    E: Send + 'static,
{
    pub fn new<F, W>(bound: usize, f: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<Message<R, S, E>>) -> W,
        W: Future<Output = Result<(), E>> + Send + 'static,
    {
        #[cfg(tokio_unstable)]
        {
            return Self::named("tower-actor-worker", bound, f);
        }

        #[cfg(not(tokio_unstable))]
        {
            let (queue_tx, queue_rx) = mpsc::channel(bound);

            tokio::spawn(f(queue_rx));

            Self {
                queue: PollSender::new(queue_tx),
            }
        }
    }

    #[cfg(tokio_unstable)]
    pub fn named<'a, F, W>(name: &'a str, bound: usize, f: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<Message<R, S, E>>) -> W,
        W: Future<Output = Result<(), E>> + Send + 'static,
    {
        let (queue_tx, queue_rx) = mpsc::channel(bound);

        tokio::task::Builder::new()
            .name(name)
            .spawn(f(queue_rx))
            .expect("failed to spawn worker");

        Self {
            queue: PollSender::new(queue_tx),
        }
    }
}

impl<R, S, E> Service<R> for Actor<R, S, E>
where
    R: Send + 'static,
    S: Send + 'static,
    E: Send + 'static + From<ActorError>,
{
    type Response = S;
    type Error = E;
    type Future = ActorFuture<S, E>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.queue
            .poll_reserve(cx)
            .map_err(|_e| ActorError::ActorTerminated.into())
    }

    fn call(&mut self, req: R) -> Self::Future {
        // Due to the permit system that PollSender uses, we can always send on the queue if
        // Poll reserve succeeded.
        // See: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Receiver.html#method.close
        //
        // Since the Service contract requires that `poll_ready()` pass
        // before calling `call()`, we can safely proceed without checking that the queue isn't closed.
        debug_assert!(!self.queue.is_closed());

        let span = Span::current();
        let (tx, rx) = oneshot::channel();

        self.queue
            .send_item(Message {
                req,
                rsp_sender: tx,
                span,
            })
            .unwrap_or_else(|e| panic!("Actor::call() called without `poll_ready`: {}", e)); // Non-debug expect()

        ActorFuture { inner: rx }
    }
}
