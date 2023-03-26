use tokio::sync::oneshot;

pub struct Message<R, S, E> {
    pub req: R,
    pub rsp_sender: oneshot::Sender<Result<S, E>>,
    pub span: tracing::Span,
}

pub trait Request: Send {
    /// Create a [`tracing::Span`] for this request, including the request name
    /// and some relevant context (but not including the entire request data).
    fn create_span(&self) -> tracing::Span;
}
