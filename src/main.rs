use std::borrow::Cow;

use failure::Error;
use tokio::stream::StreamExt;

mod async_io;
mod init_info;
mod pruning;
mod rpc;
mod stdio;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

fn is_onion(url: &reqwest::Url) -> bool {
    if let Some(url::Host::Domain(s)) = url.host() {
        s.ends_with(".onion")
    } else {
        false
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logging::log_to_stderr(log::LevelFilter::Info); // set up logging

    // start rpc handler and wait for info needed from "init" method
    let (sender, reciever) = crossbeam_channel::bounded(1);
    let rpc_handler = std::thread::spawn(move || stdio::run_rpc_handler(sender));
    let init_info = init_info::InitInfoArc::new(reciever).wait_for_info().await;

    // connect an RPC socket to be reused for rpc requests
    let mut socket = tokio::net::UnixStream::connect(&*init_info.socket_path).await?;

    // fetch configuration params external to the plugin
    let config_info = rpc::make_socket_req(
        &mut socket,
        rpc::RpcReq {
            id: Some(rpc::JsonRpcV2Id::Num(0.into())),
            jsonrpc: Default::default(),
            method: Cow::Borrowed("listconfigs"),
            params: rpc::RpcParams::ByPosition(Vec::new()),
        },
    )
    .await?
    .result
    .res()?;
    let config_info: init_info::ConfigInfo = serde_json::from_value(config_info)?;
    let bitcoin_info: init_info::BitcoinInfo = serde_json::from_value(
        config_info
            .plugins
            .into_iter()
            .filter(|a| &a.name == "bcli")
            .next()
            .ok_or_else(|| failure::format_err!("bcli info not found"))?
            .options,
    )?;

    // create an http request to bitcoind that can be cloned and reused
    let client = reqwest::Client::builder().user_agent(APP_USER_AGENT);
    let client = if let Some(proxy) = config_info.proxy {
        // use provided socks5 proxy if necessary
        let proxy = reqwest::Url::parse(&format!("socks5h://{}", proxy))?;
        client.proxy(if config_info.always_use_proxy {
            reqwest::Proxy::all(proxy)?
        } else {
            reqwest::Proxy::custom(move |url| {
                if is_onion(&url) {
                    Some(proxy.clone())
                } else {
                    None
                }
            })
        })
    } else {
        client
    };
    let client = client.build()?;
    let mut bitcoin_url = reqwest::Url::parse("http://localhost")?;
    bitcoin_url.set_host(Some(&format!("{}", bitcoin_info.bitcoin_rpcconnect)))?;
    bitcoin_url
        .set_port(Some(
            bitcoin_info
                .bitcoin_rpcport
                .unwrap_or(config_info.network.default_port()),
        ))
        .map_err(|_| failure::format_err!("unable to set port"))?;
    let bitcoin_req = client.post(bitcoin_url).basic_auth(
        bitcoin_info.bitcoin_rpcuser,
        Some(bitcoin_info.bitcoin_rpcpassword),
    );

    // every `pruning-interval` seconds, run the `prune` method
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(init_info.pruning_interval));
    while let Some(_) = interval.next().await {
        match pruning::prune(&mut socket, &bitcoin_req, config_info.rescan).await {
            Ok(_) => (),
            Err(e) => log::error!("{}", e),
        }
    }

    rpc_handler.join().unwrap();

    Ok(())
}
