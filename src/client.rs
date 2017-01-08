use bincode::SizeLimit;
use bincode::serde::{deserialize, serialize};
use crypto;
use crypto::{Key, Sealed};
use futures::{Sink, Stream, Future};
use futures::sync::mpsc;
use rustc_serialize::hex::FromHex;
use rustc_serialize::hex::ToHex;
use std::cell::Cell;
use std::env;
use std::error;
use std::fs;
use std::io;
use std::net;
use std::net::SocketAddr;
use std::process;
use std::time::Duration;
use std::time::Instant;
use tokio_core::net::{UdpSocket, UdpCodec};
use tokio_core::reactor::{Core, Interval};
use types::{MoquClientReq, MoquUpdate, MoquItem};

pub struct ClientCodec {
    key: Key,
}

impl UdpCodec for ClientCodec {
    type In = (SocketAddr, MoquUpdate);
    type Out = (SocketAddr, MoquClientReq);

    fn decode(&mut self, addr: &SocketAddr, buf: &[u8]) -> io::Result<Self::In> {
        let sealed: Sealed =
            deserialize(buf).map_err(|_e|
                    io::Error::new(io::ErrorKind::InvalidData,
                                   "Malformed envelope"))?;
        let opened = sealed.open(&self.key)
            .map_err(|_e| io::Error::new(io::ErrorKind::InvalidData, "Couldn't decrypt update"))?;
        Ok((*addr,
         deserialize(&opened)
         .map_err(|_e| io::Error::new(io::ErrorKind::InvalidData,
                                      "Couldn't parse update"))?))
    }

    fn encode(&mut self,
              (addr, request): (SocketAddr, MoquClientReq),
              into: &mut Vec<u8>)
              -> SocketAddr {
        let plaintext = serialize(&request, SizeLimit::Infinite).unwrap();
        let sealed = crypto::seal(&self.key, &plaintext[..]).unwrap();
        let mut sealed_serialized = serialize(&sealed, SizeLimit::Infinite).unwrap();
        into.append(&mut sealed_serialized);
        addr
    }
}

pub fn handle_item(item: &MoquItem) -> Result<(), Box<error::Error>> {
    let handler_path = format!("./handle.{}", item.kind);
    let handlerstat = fs::metadata(handler_path.clone())?;
    if handlerstat.is_file() {
        process::Command::new(handler_path).stdout(process::Stdio::inherit())
            .arg(item.content.clone())
            .spawn()?;
    }
    Ok(())
}

pub fn client(server: &str, sport: u16, ipv6: bool) -> Result<(), Box<error::Error>> {
    let mut cliseq: u64 = 0;
    // have seen the first seq (for reconnecting.)
    let mut ready: bool = false;
    let last_heartbeat: Cell<Option<Instant>> = Cell::new(None);
    let keybytes: Vec<u8> = env::var("MOQU_KEY").unwrap().from_hex().unwrap();
    let mut core = Core::new()?;
    let handle = core.handle();
    let key = Key::from_bytes(&keybytes);
    println!("Client Key: MOQU_KEY={}", key.bytes.to_hex());
    let conaddr = if ipv6 { "::" } else { "0.0.0.0" };
    let consock = net::UdpSocket::bind((conaddr, 0))?;
    let bound_addr = consock.local_addr()?;
    let serveraddr = net::ToSocketAddrs::to_socket_addrs(&(server, sport))?
        .next()
        .ok_or("Couldn't construct server socket addr")?;
    println!("Server: {:?}", serveraddr);
    let tsock = UdpSocket::from_socket(consock, &handle)?;
    println!("Client addr: {:?}", bound_addr);
    let framed = tsock.framed(ClientCodec { key: key });
    let (requests, updates) = framed.split();
    let (sendqueue, outstream) = mpsc::unbounded();
    handle.spawn(requests.send_all(outstream.map_err(|_| io::ErrorKind::Other)).then(|_| Ok(())));

    let heartbeat_interval = Interval::new(Duration::from_millis(1500), &handle)?;
    let heartbeater = heartbeat_interval.for_each(|_| {
        if let Some(time) = last_heartbeat.get() {
            if time.elapsed().as_secs() > 10 {
                handle.spawn(sendqueue.clone()
                    .send((serveraddr, MoquClientReq::AddrUpdate))
                    .then(|_| Ok(())));
            }
        } else {
            handle.spawn(sendqueue.clone()
                .send((serveraddr, MoquClientReq::AddrUpdate))
                .then(|_| Ok(())));
        }
        Ok(())
    });

    let done = updates.for_each(|(_addr, update)| {
        match update {
            MoquUpdate::Heartbeat(time) => {
                info!("Got server heartbeat (time: {})", time);
                handle.spawn(sendqueue.clone()
                    .send((serveraddr, MoquClientReq::Watermark(cliseq)))
                    .then(|_| Ok(())));
                last_heartbeat.set(Some(Instant::now()));
            }
            MoquUpdate::Item(seq, item) => {
                info!("Got item: {:?} (seq {})", item, seq);
                if seq == cliseq + 1 {
                    ready = true;
                    cliseq += 1;
                    if let Err(e) = handle_item(&item) {
                        error!("Error handling item: {:?}", e);
                    };
                    handle.spawn(sendqueue.clone()
                        .send((serveraddr, MoquClientReq::Watermark(cliseq)))
                        .then(|_| Ok(())));
                } else {
                    if !ready {
                        info!("Reconnect (iseq: {}, cliseq: {})", seq, cliseq);
                        ready = true;
                        cliseq = seq - 1;
                    } else {
                        warn!("out of order? (iseq: {}, cliseq: {})", seq, cliseq);
                    }
                }
            }
        }
        Ok(())
    });
    Ok(core.run(done.join(heartbeater)).map(|_| ())?)
}

pub fn publish(server: &str,
               sport: u16,
               ipv6: bool,
               item: MoquItem)
               -> Result<(), Box<error::Error>> {
    let keybytes: Vec<u8> = env::var("MOQU_KEY").unwrap().from_hex().unwrap();
    let key = Key::from_bytes(&keybytes);
    let conaddr = if ipv6 { "::" } else { "0.0.0.0" };
    let consock = net::UdpSocket::bind((conaddr, 0))?;
    let serveraddr = net::ToSocketAddrs::to_socket_addrs(&(server, sport))?
        .next()
        .ok_or("Couldn't construct server socket addr")?;
    println!("Server: {:?}", serveraddr);
    let mut codec = ClientCodec { key: key };
    let mut message = Vec::new();
    codec.encode((serveraddr, MoquClientReq::Publish(item)), &mut message);
    consock.send_to(&message, serveraddr)?;
    Ok(())
}
