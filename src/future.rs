use std::task::{Context, Poll};

use futures::Future;
use pin_project::pin_project;

use tokio::sync::oneshot;

use crate::ActorError;

#[pin_project(project = ActorFutureProjection)]
pub struct ActorFuture<S, E> {
    #[pin]
    pub(crate) inner: oneshot::Receiver<Result<S, E>>,
}

impl<S, E> Future for ActorFuture<S, E>
where
    E: From<ActorError>,
{
    type Output = Result<S, E>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().inner.poll(cx) {
            Poll::Ready(Ok(res)) => Poll::Ready(res),
            Poll::Ready(Err(_)) => Poll::Ready(Err(ActorError::ActorHungUp.into())),
            Poll::Pending => Poll::Pending,
        }
    }
}
