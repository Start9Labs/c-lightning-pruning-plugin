use std::borrow::Cow;

use failure::Error;
use tokio::stream::StreamExt;

mod async_io;
mod init_info;
mod pruning;
mod rpc;
mod stdio;

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logging::log_to_stderr(log::LevelFilter::Info);

    let (sender, reciever) = crossbeam_channel::bounded(1);
    let rpc_handler = std::thread::spawn(move || stdio::run_rpc_handler(sender));
    let init_info = init_info::InitInfoArc::new(reciever).wait_for_info().await;

    let mut socket = tokio::net::UnixStream::connect(&*init_info.socket_path).await?;
    let bitcoin_info = rpc::make_socket_req(
        &mut socket,
        rpc::RpcReq {
            id: Some(rpc::JsonRpcV2Id::Num(0.into())),
            jsonrpc: Default::default(),
            method: Cow::Borrowed("listconfigs"),
            params: rpc::RpcParams::ByPosition(Vec::new()),
        },
    )
    .await?;
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(init_info.pruning_interval));
    while let Some(_) = interval.next().await {
        match pruning::prune(&mut socket).await {
            Ok(_) => (),
            Err(e) => log::error!("{}", e),
        }
    }

    rpc_handler.join().unwrap();

    Ok(())
}
