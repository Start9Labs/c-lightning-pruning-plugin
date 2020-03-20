use failure::Error;
use tokio::net::UnixStream;

use crate::rpc::{make_socket_req, RpcReq};

pub async fn prune(socket: &mut UnixStream) -> Result<(), Error> {
    Ok(())
}
