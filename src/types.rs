
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MoquItem {
    pub kind: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MoquUpdate {
    Heartbeat(u64),
    Item(u64, MoquItem),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MoquClientReq {
    AddrUpdate,
    Publish(MoquItem),
    Watermark(u64),
}
