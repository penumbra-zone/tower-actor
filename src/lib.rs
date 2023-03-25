mod actor;
mod future;
mod message;

pub use actor::*;

#[cfg(test)]
mod tests {
    use crate::{actor::Actor, message::Request};
    use tower::Service;

    impl Request for () {
        fn create_span(&self) -> tracing::Span {
            unimplemented!()
        }
    }

    #[test]
    #[should_panic]
    fn follows_service_contract() {
        let mut actor = Actor::<(), (), ()>::new(0, |_| async move { Ok(()) });
        actor.call(());
    }
}
