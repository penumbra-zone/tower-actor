mod actor;
mod future;
mod message;

pub use actor::*;
pub use future::*;
pub use message::*;

#[cfg(test)]
mod tests {
    use crate::actor::Actor;
    use crate::ActorError;
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use thiserror::Error;
    use tokio::sync::{oneshot, Mutex};

    use tower::Service;

    #[derive(Error, Debug, PartialEq, Eq)]
    enum TestError {
        #[error("Poll Error")]
        ActorTerminated,
        #[error("Poll Sender error")]
        ActorHungUp,
    }

    impl From<ActorError> for TestError {
        fn from(value: ActorError) -> Self {
            match value {
                ActorError::ActorTerminated => TestError::ActorTerminated,
                ActorError::ActorHungUp => TestError::ActorHungUp,
            }
        }
    }

    #[test]
    #[should_panic]
    fn follows_service_contract() {
        let mut actor = Actor::<(), (), TestError>::new(10, |_| async move { Ok(()) });
        actor.call(());
    }

    // Test misbehaving actor dropping the call
    #[tokio::test]
    async fn test_misbehaving_actor() {
        let mut actor = Actor::<(), (), TestError>::new(10, |mut rx| async move {
            let msg = rx.recv().await;
            drop(msg);
            Ok(())
        });
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(actor.call(()).await, Err(TestError::ActorHungUp));
    }

    // Test failed actor crashing the receiver on another thread
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_crashing_actor() {
        let mut actor = Actor::<(), (), TestError>::new(10, |mut rx| async move {
            let _msg = rx.recv().await;
            panic!();
        });
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(actor.call(()).await, Err(TestError::ActorHungUp));
    }

    // Test failed actor crashing on another thread before processing messages
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_fast_crashing_actor() {
        let mut actor = Actor::<(), (), TestError>::new(10, |mut _rx| async move {
            panic!();
        });
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        assert_eq!(
            actor.poll_ready(&mut cx),
            Poll::Ready(Err(TestError::ActorTerminated))
        );
    }

    // Test buffer full
    #[tokio::test]
    async fn test_actor_pending() {
        let (outer_tx, outer_rx) = oneshot::channel::<()>();
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        let mut actor = Actor::<(), (), TestError>::new(1, |mut rx| async move {
            // Wait to complete our first message until signaled
            outer_rx.await.unwrap();

            let msg = rx.recv().await.unwrap();
            msg.rsp_sender.send(Ok(())).unwrap();

            // Make sure we can receive one more message for the last poll_ready
            rx.recv().await;
            Ok(())
        });

        // Prep the actor
        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));

        // Launch the message
        let fut = actor.call(());

        // Observe the pending status
        assert_eq!(actor.poll_ready(&mut cx), Poll::Pending);

        // Complete the message
        outer_tx.send(()).unwrap();

        // Observe the value and actor are OK
        assert_eq!(fut.await, Ok(()));
        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));
    }

    // End to end test of 'happy path' where Actor receives messages and can mutate itself
    #[tokio::test]
    async fn test_happy_path() {
        let mut actor = Actor::<u32, u32, TestError>::new(10, |mut rx| async move {
            // Simulate some actor state...
            let counter = Arc::new(Mutex::new(0));

            while let Some(msg) = rx.recv().await {
                let rsp_sender = msg.rsp_sender;
                let counter = counter.clone();
                let new_count = msg.req;

                // Which farms out the work to another thread
                tokio::spawn(async move {
                    let mut lock = counter.lock().await;
                    *lock += new_count;
                    rsp_sender.send(Ok(*lock))
                });
            }
            Ok(())
        });
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());

        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(actor.call(10).await, Ok(10));

        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(actor.call(20).await, Ok(30));

        assert_eq!(actor.poll_ready(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(actor.call(30).await, Ok(60));
    }
}
