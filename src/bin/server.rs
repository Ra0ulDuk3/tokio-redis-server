use bytes::Bytes;
use mini_redis::Command::{self, Get, Set};
use mini_redis::{Connection, Frame};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main() {
    // bind tcp listener to localhost on port 6379, then operation is
    // ran asynchronously using await
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    println!("Listening");

    // create the database
    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        // dropping second item since we dont need it now
        // (it is the ip and port of the connection)
        let (socket, _) = listener.accept().await.unwrap();

        // clone the handle, kind of like a reference
        let db = db.clone();

        // spawn a new task for each inbound socket,
        // moving socket into the new task
        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}

async fn process(socket: TcpStream, db: Db) {
    // this lets us read/write **frames** instead of byte streams
    let mut connection = Connection::new(socket);

    // receive commands using read_frame
    while let Some(frame) = connection.read_frame().await.unwrap() {
        /*
         * The following match expression grabs the commands using from_frame
         * (the data sent over to our server)
         * and checks if its the `Set(command)` or the `Get(command)` function
         * call
         */
        let response = match Command::from_frame(frame).unwrap() {
            // we want to update the values
            Set(cmd) => {
                // lock the db mutex and release the variable into `db`
                let mut db = db.lock().unwrap();
                // store key as string and val as vec
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            // we want to read the values
            Get(cmd) => {
                // notice the lack of mut, since we only want to read
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    // pass our bytes in, we didnt store as a vec so no
                    // need to convert using `into()`
                    Frame::Bulk(value.clone())
                } else {
                    Frame::Null
                }
            }
            // {:?} allows for printing with the debug format, only
            // valid if frame implements the `#[derive(Debug)]` trait
            cmd => panic!("unimplemented {:?}", cmd),
        };
        // write response to the client
        connection.write_frame(&response).await.unwrap();
    }
}
