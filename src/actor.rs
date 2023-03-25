use std::{
    future::Future,
    task::{Context, Poll},
};

use crate::future::ActorFuture;
use crate::message::{Message, Request};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::PollSender;
use tower::Service;

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("Actor has terminated or panicked")]
    ActorTerminated,
    #[error("Actor has terminated this request without responding")]
    RequestCancelled,
}

pub struct Actor<R, S, E> {
    queue: tokio_util::sync::PollSender<Message<R, S, E>>,
}

impl<R, S, E> Actor<R, S, E>
where
    R: Request + 'static,
    S: Send + 'static,
    E: Send + 'static,
{
    pub fn new<F, W>(bound: usize, f: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<Message<R, S, E>>) -> W,
        W: Future<Output = Result<S, E>> + Send + 'static,
    {
        Self::named("tower-actor-worker", bound, f)
    }

    pub fn named<'a, F, W>(name: &'a str, bound: usize, f: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<Message<R, S, E>>) -> W,
        W: Future<Output = Result<S, E>> + Send + 'static,
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
    R: Request + 'static,
    S: Send + 'static,
    E: Send + 'static,
{
    type Response = Result<S, E>;
    type Error = ActorError;
    type Future = ActorFuture<S, E>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.queue
            .poll_reserve(cx)
            .map_err(|_| ActorError::ActorTerminated)
    }

    fn call(&mut self, req: R) -> Self::Future {
        if self.queue.is_closed() {
            return ActorFuture::Terminated;
        }

        let span = req.create_span();
        let (tx, rx) = oneshot::channel();

        self.queue
            .send_item(Message {
                req,
                rsp_sender: tx,
                span,
            })
            .unwrap_or_else(|e| panic!("called without `poll_ready`: {}", e)); // Non-debug requiring expect

        ActorFuture::Future(rx)
    }
}
