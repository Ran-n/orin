// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:20.778140
//+ Revised: 	2026/11/12 09:16:51.625314
// -----------------------------------------------------------------------------

use std::{
    io::{BufRead, BufReader, Read},
    sync::mpsc::{self, Receiver},
    thread,
};

use serde_json::Value;

pub fn response_reader(output: impl Read + Send + 'static) -> Receiver<Value> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { break };
            if let Some(response) = line.strip_prefix("@@") {
                let Ok(response) = serde_json::from_str(response) else {
                    break;
                };
                if sender.send(response).is_err() {
                    break;
                }
            }
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, time::Duration};

    #[test]
    fn response_reader_ignores_non_protocol_output() {
        let responses = response_reader(Cursor::new(
            b"startup diagnostic\n@@{\"id\":1,\"ok\":true}\n",
        ));
        assert_eq!(
            responses.recv_timeout(Duration::from_secs(1)).unwrap(),
            serde_json::json!({"id":1,"ok":true})
        );
    }

    #[test]
    fn response_reader_rejects_invalid_json() {
        let responses = response_reader(Cursor::new(b"@@invalid json\n"));
        assert!(responses.recv_timeout(Duration::from_secs(1)).is_err());
    }
}
