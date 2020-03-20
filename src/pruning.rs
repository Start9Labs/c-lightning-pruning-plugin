use std::borrow::Cow;

use failure::Error;
use serde_json::Value;
use tokio::net::UnixStream;

use crate::rpc::{make_socket_req, JsonRpcV2Id, RpcParams, RpcReq};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct LightningInfo {
    pub blockheight: u64,
}

pub async fn prune(
    socket: &mut UnixStream,
    bitcoin_req: &reqwest::RequestBuilder,
    rescan: u64,
) -> Result<(), Error> {
    let res = make_socket_req(
        socket,
        RpcReq {
            id: Some(JsonRpcV2Id::Num(0.into())),
            jsonrpc: Default::default(),
            method: Cow::Borrowed("getinfo"),
            params: RpcParams::ByPosition(Vec::new()),
        },
    )
    .await?
    .result
    .res()?;
    let res: LightningInfo = serde_json::from_value(res)?;
    bitcoin_req
        .try_clone()
        .ok_or_else(|| failure::format_err!("cannot clone request"))?
        .json(&RpcReq {
            id: Some(JsonRpcV2Id::Num(0.into())),
            jsonrpc: Default::default(),
            method: Cow::Borrowed("pruneblockchain"),
            params: RpcParams::ByPosition(vec![Value::Number((res.blockheight - rescan).into())]),
        })
        .send()
        .await?;

    Ok(())
}
