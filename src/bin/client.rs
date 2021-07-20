use bytes::Bytes;
use mini_redis::client;
use tokio::sync::{oneshot, mspc};

// give derive(Debug) trait so we can print Command enums
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key:String,
        val: Bytes,
        resp: Responder<()>;
    }
}

// Provided by the requester and used by manager task to send 
// the command response back to the requester.
type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

// wrap in synchronous runtime
#[tokio::main]
async fn main() {
    // create a new channel with a capacity of at most 32 connections
    let (tx, mut rx) = mpsc::channel(32);

    /* the manager task is going to be used to receive 
    * and process requests by sending them one by one 
    * to the redis client
    */
    let manager = tokio::spawn(async move { 
        // establish a connection to the server, only one we need
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        // begin receiving messages
        while let Some(cmd) = rx.recv().await {
            use Command::*;

            match cmd {
                Get { key, resp } => {
                    let res = client.get(&key).await;
                    // ignore unused var error
                    let _ = resp.send(res);
                }
                Set { key, val, resp } => {
                    let res = client.set(&key, val).await;
                    // ignore unused var error
                    // NOTE: calling send on oneshot::sender completes immediately and doesn't
                    // require a wait
                    let _ = resp.send(res);
            }
        }
    });


    let tx2 = tx.clone();
    // spawn first task to get an entry
    let task1 = tokio::spawn(async {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Get {
            key: "hello".to_string(),
            // give sender so manager can send us stuff
            resp: resp_tx,
        };
        tx.send(cmd).await.unwrap();

        // wait for the response from manager
        let res = resp_rx.await;
        println!("Got = {:?}", res);
    });

    // spawn second task, to set an entry
    let task2 = tokio::spawn(async {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Set {
            key: "foo".to_string(),
            val: "bar".into(),
            resp: resp_tx,
        };

        // send request
        tx2.send(cmd).await.unwrap();
        // wait for response
        let res = resp_rx.await;
        // process response, since this is executed after we have waited
        println!("Got = {:?}", res);
    });

    task1.await.unwrap();
    task2.await.unwrap();
    manager.await.unwrap();
