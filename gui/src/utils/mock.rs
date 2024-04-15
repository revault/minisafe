use std::fmt::Debug;
use std::str::FromStr;
use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Mutex,
};
use std::thread;

use liana::descriptors::LianaDescriptor;

use crate::app::wallet::Wallet;
use crate::daemon::{client::Client, DaemonError};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

type TransportReceiver = Receiver<Result<Value, DaemonError>>;

#[derive(Debug)]
pub struct DaemonClient {
    transport: Mutex<(Sender<Value>, TransportReceiver)>,
}

impl Client for DaemonClient {
    type Error = DaemonError;
    fn request<S: Serialize + Debug, D: DeserializeOwned + Debug>(
        &self,
        method: &str,
        params: Option<S>,
    ) -> Result<D, Self::Error> {
        let req = json!({"method": method, "params": params});
        let connection = self.transport.lock().expect("Failed to unlock");
        connection
            .0
            .send(req)
            .expect("Mock client failed to send request");
        connection
            .1
            .recv()
            .expect("Mock client failed to receive response")
            .map(|value| serde_json::from_value(value).unwrap())
    }
}

pub struct Daemon {
    requests: Vec<(Option<Value>, Result<Value, DaemonError>)>,
}

impl Daemon {
    pub fn new(requests: Vec<(Option<Value>, Result<Value, DaemonError>)>) -> Self {
        Self { requests }
    }

    pub fn run(self) -> DaemonClient {
        let (client_sender, daemon_receiver) = channel();
        let (daemon_sender, client_receiver) = channel();

        thread::spawn(move || {
            let mut requests = self.requests.into_iter();
            while let Ok(msg) = daemon_receiver.recv() {
                let request = requests
                    .next()
                    .expect("Mock Daemon must have all requests mocked in the right order");
                if let Some(body) = request.0 {
                    assert_eq!(body, msg);
                }
                daemon_sender
                    .send(request.1)
                    .expect("Mock daemon failed to send response")
            }
            // close the daemon -> client channel after
            // the client -> daemon channel is closed.
            // (client -> daemon channel is closed when DaemonClient is dropped)
            drop(daemon_sender);
            // Readable with `cargo test -- --nocapture`
            println!("The daemon has stopped!");
        });

        DaemonClient {
            transport: Mutex::new((client_sender, client_receiver)),
        }
    }
}

const DESC: &str = "wsh(or_d(multi(2,[ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/<0;1>/*,[de6eb005/48'/1'/0'/2']tpubDFGuYfS2JwiUSEXiQuNGdT3R7WTDhbaE6jbUhgYSSdhmfQcSx7ZntMPPv7nrkvAqjpj3jX9wbhSGMeKVao4qAzhbNyBi7iQmv5xxQk6H6jz/<0;1>/*),and_v(v:pkh([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/<2;3>/*),older(3))))#p9ax3xxp";

pub fn mock_wallet() -> Wallet {
    Wallet::new(mock_descriptor())
}

pub fn mock_descriptor() -> LianaDescriptor {
    LianaDescriptor::from_str(DESC).unwrap()
}
