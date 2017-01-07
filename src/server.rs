use bincode::SizeLimit;
use bincode::serde::{deserialize, serialize};
use crypto;
use crypto::{Key, Sealed};
use futures::{Sink, Stream, Future};
use futures::sync::mpsc;
use rustc_serialize::hex::FromHex;
use rustc_serialize::hex::ToHex;
use std::cell::Cell;
use std::collections::VecDeque;
use std::env;
use std::error;
use std::io;
use std::net;
use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;
use tokio_core::net::{UdpSocket, UdpCodec};
use tokio_core::reactor::{Core, Interval};
use types::{MoquClientReq, MoquUpdate, MoquItem};

pub struct ServerCodec {
    key: Key,
}

impl ServerCodec {
    fn maybe_decode(&mut self, buf: &[u8]) -> io::Result<MoquClientReq> {
        let sealed: Sealed =
        deserialize(buf).map_err(|_e|
                io::Error::new(io::ErrorKind::InvalidData,
                               "Malformed envelope"))?;
        let opened = sealed.open(&self.key)
            .map_err(|_e| io::Error::new(io::ErrorKind::InvalidData, "Couldn't decrypt request"))?;
        Ok(deserialize(&opened).map_err(|_e| io::Error::new(io::ErrorKind::InvalidData,
                                  "Couldn't parse request"))?)
    }
}

impl UdpCodec for ServerCodec {
    type In = (SocketAddr, io::Result<MoquClientReq>);
    type Out = (SocketAddr, MoquUpdate);

    fn decode(&mut self, addr: &SocketAddr, buf: &[u8]) -> io::Result<Self::In> {
        Ok((*addr, self.maybe_decode(buf)))
    }

    fn encode(&mut self,
              (addr, update): (SocketAddr, MoquUpdate),
              into: &mut Vec<u8>)
              -> SocketAddr {
        let plaintext = serialize(&update, SizeLimit::Infinite).unwrap();
        let sealed = crypto::seal(&self.key, &plaintext[..]).unwrap();
        let mut sealed_serialized = serialize(&sealed, SizeLimit::Infinite).unwrap();
        into.append(&mut sealed_serialized);
        addr
    }
}

struct MoquQueue {
    queue: VecDeque<(u64, MoquItem)>,
    sseq: u64,
    cliseq: u64,
}

impl MoquQueue {
    fn new() -> MoquQueue {
        MoquQueue {
            queue: VecDeque::new(),
            sseq: 0,
            cliseq: 0,
        }
    }

    fn insert(&mut self, value: MoquItem) {
        self.sseq += 1;
        self.queue.push_back((self.sseq, value));
    }

    fn front(&self) -> Option<&(u64, MoquItem)> {
        self.queue.front()
    }

    fn pop(&mut self) {
        if let Some(_) = self.queue.pop_front() {
            self.cliseq += 1;
        }
    }

    fn pop_until(&mut self, seq: u64) {
        while self.cliseq < seq {
            self.pop();
        }
    }
}

pub fn serve(port: u16) -> Result<(), Box<error::Error>> {
    let mut queue = MoquQueue::new();
    let mut core = Core::new()?;
    let handle = core.handle();
    let key = match env::var("MOQU_KEY") {
        Ok(keystr) => Key::from_bytes(&keystr.from_hex().unwrap()),
        Err(_) => Key::new()?,
    };
    println!("Key: MOQU_KEY={}", key.bytes.to_hex());
    let consock = net::UdpSocket::bind(("0.0.0.0", port))?;
    let tsock = UdpSocket::from_socket(consock, &handle)?;
    println!("Listening on port {}", port);
    let framed = tsock.framed(ServerCodec { key: key });
    let (updates, requests) = framed.split();
    let (sendqueue, outstream) = mpsc::unbounded();
    let starttime = Instant::now();
    let clientaddr = Cell::new(None);
    handle.spawn(updates.send_all(outstream.map_err(|_| io::ErrorKind::Other)).then(|_| Ok(())));
    let heartbeat_interval = Interval::new(Duration::from_millis(1500), &handle)?;
    let heartbeater = heartbeat_interval.for_each(|_| {
        if let Some(addr) = clientaddr.get() {
            handle.spawn(sendqueue.clone()
                .send((addr, MoquUpdate::Heartbeat(starttime.elapsed().as_secs())))
                .then(|_| Ok(())));
        }
        Ok(())
    });
    let done = requests.for_each(|(addr, decode_result)| {
        match decode_result {
            Ok(req) => {
                match req {
                    MoquClientReq::AddrUpdate => {
                        info!("Got new client address: {:?}", addr);
                        clientaddr.set(Some(addr));
                    }
                    MoquClientReq::Publish(item) => {
                        info!("Enqueuing item: {:?}", item);
                        queue.insert(item);
                    }
                    MoquClientReq::Watermark(seq) => {
                        info!("Got client watermark: {:?}", seq);
                        queue.pop_until(seq);
                        if let Some(&(seq, ref item)) = queue.front() {
                            handle.spawn(sendqueue.clone()
                                .send((addr, MoquUpdate::Item(seq, item.clone())))
                                .then(|_| Ok(())));
                        }
                    }
                }
            }
            Err(e) => error!("Error decoding request: {}", e),
        }
        Ok(())
    });
    Ok(core.run(done.join(heartbeater)).map(|_| ())?)
}
