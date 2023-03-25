use std::task::{Context, Poll};

use futures::Future;
use pin_project::pin_project;

use tokio::sync::oneshot::{self, error::RecvError};

use crate::ActorError;

impl From<RecvError> for ActorError {
    fn from(_: RecvError) -> Self {
        ActorError::RequestCancelled
    }
}

#[pin_project(project = ActorFutureProjection)]
pub enum ActorFuture<S, E> {
    Future(#[pin] oneshot::Receiver<Result<S, E>>),
    Terminated,
}

impl<S, E> Future for ActorFuture<S, E> {
    type Output = Result<Result<S, E>, ActorError>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            ActorFutureProjection::Future(inner) => inner.poll(cx).map_err(Into::into),
            ActorFutureProjection::Terminated => Poll::Ready(Err(ActorError::ActorTerminated)),
        }
    }
}
