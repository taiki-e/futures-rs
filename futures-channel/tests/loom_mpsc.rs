#![cfg(futures_loom)]

use futures::{future::poll_fn, sink::SinkExt, stream::StreamExt};
use futures_channel::mpsc;
use loom::future::block_on;
use loom::sync::Arc;
use loom::thread;

#[test]
fn closing_tx() {
    loom::model(|| {
        let (mut tx, mut rx) = mpsc::channel(16);

        thread::spawn(move || {
            tx.try_send(()).unwrap();
            drop(tx);
        });

        let v = block_on(rx.next());
        assert!(v.is_some());

        let v = block_on(rx.next());
        assert!(v.is_none());
    });
}

// #[test]
// fn closing_unbounded_tx() {
//     loom::model(|| {
//         let (mut tx, mut rx) = mpsc::unbounded();

//         thread::spawn(move || {
//             block_on(tx.send(())).unwrap();
//             drop(tx);
//         });

//         let v = block_on(rx.next());
//         assert!(v.is_some());

//         let v = block_on(rx.next());
//         assert!(v.is_none());
//     });
// }

// #[test]
// fn closing_bounded_rx() {
//     loom::model(|| {
//         let (mut tx1, rx) = mpsc::channel::<()>(16);
//         let tx2 = tx1.clone();
//         thread::spawn(move || {
//             drop(rx);
//         });

//         tx1.close_channel();
//         tx2.close_channel();
//     });
// }

// #[test]
// fn closing_and_sending() {
//     loom::model(|| {
//         let (mut tx1, mut rx) = mpsc::channel::<()>(16);
//         let tx2 = tx1.clone();

//         let th1 = thread::spawn(move || {
//             block_on(tx1.send(())).unwrap();
//         });

//         let th2 = thread::spawn(move || {
//             tx2.close_channel();
//         });

//         let th3 = thread::spawn(move || {
//             let v = block_on(rx.next());
//             assert!(v.is_some());
//             drop(rx);
//         });

//         th1.join().unwrap();
//         th2.join().unwrap();
//         th3.join().unwrap();
//     });
// }

// #[test]
// fn closing_unbounded_rx() {
//     loom::model(|| {
//         let (mut tx1, rx) = mpsc::unbounded::<()>();
//         let tx2 = tx1.clone();
//         thread::spawn(move || {
//             drop(rx);
//         });

//         tx1.close_channel();
//         tx2.close_channel();
//     });
// }

// #[test]
// fn dropping_tx() {
//     loom::model(|| {
//         let (mut tx, mut rx) = mpsc::channel::<()>(16);

//         for _ in 0..2 {
//             let tx = tx.clone();
//             thread::spawn(move || {
//                 drop(tx);
//             });
//         }
//         drop(tx);

//         let v = block_on(rx.next());
//         assert!(v.is_none());
//     });
// }

// #[test]
// fn dropping_unbounded_tx() {
//     loom::model(|| {
//         let (mut tx, mut rx) = mpsc::unbounded::<()>();

//         for _ in 0..2 {
//             let tx = tx.clone();
//             thread::spawn(move || {
//                 drop(tx);
//             });
//         }
//         drop(tx);

//         let v = block_on(rx.next());
//         assert!(v.is_none());
//     });
// }
