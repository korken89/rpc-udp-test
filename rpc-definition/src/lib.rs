#![no_std]

pub use postcard_rpc;

use postcard::experimental::schema::Schema;
use serde::{Deserialize, Serialize};

pub mod topics {
    pub mod some_data {
        use super::super::*;
        use postcard_rpc::topic;

        topic!(TopicSomeData, SomeData, "topic/somedata");

        #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
        pub struct SomeData {
            pub value: f32,
        }
    }
}

pub mod endpoints {
    pub mod sleep {
        use postcard_rpc::endpoint;

        use super::super::*;

        endpoint!(SleepEndpoint, Sleep, SleepDone, "endpoint/sleep");

        #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
        pub struct Sleep {
            pub seconds: u32,
            pub micros: u32,
        }

        #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
        pub struct SleepDone {
            pub slept_for: Sleep,
        }
    }
}

pub mod wire_error {
    use postcard_rpc::Key;

    use super::*;

    pub const ERROR_PATH: &str = "error";
    pub const ERROR_KEY: Key = Key::for_path::<FatalError>(ERROR_PATH);

    /// Fatal errors on the embedded device.
    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    pub enum FatalError {
        /// We're asking for an endpoint the embedded device does not know about.
        UnknownEndpoint,
        /// The internal dispatcher in the embedded device is full of requests and can't enqueue.
        NotEnoughSenders,
        /// Ser(/de) error, malformed packet.
        WireFailure,
    }
}
