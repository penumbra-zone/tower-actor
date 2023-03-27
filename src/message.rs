use tokio::sync::oneshot;

#[derive(Debug)]
pub struct Message<R, S, E> {
    pub req: R,
    pub rsp_sender: oneshot::Sender<Result<S, E>>,
    pub span: tracing::Span,
}
