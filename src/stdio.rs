use std::borrow::Borrow;
use std::borrow::Cow;
use std::path::PathBuf;

use crossbeam_channel::Sender;
use serde_json::StreamDeserializer;
use serde_json::Value;

use crate::init_info::InitInfo;
use crate::rpc::*;

pub fn handle_init(sender: &Sender<InitInfo>, params: &RpcParams) -> Result<Value, RpcError> {
    let arg0 = match params {
        RpcParams::ByPosition(a) => a
            .get(0)
            .ok_or(RpcError {
                code: 4.into(),
                message: Cow::Borrowed("no arguments supplied"),
                data: None,
            })?
            .clone(),
        RpcParams::ByName(a) => serde_json::Value::Object(a.clone()),
    };
    let conf: LightningInit = serde_json::from_value(arg0)
        .map_err(|e| format!("{}", e))
        .with_info(5, "params deserialization error")?;
    sender
        .send(conf.into())
        .unwrap_or_else(|e| log::warn!("SEND ERROR: {}", e)); // ignore send error: means the reciever has already received and been dropped
    Ok(serde_json::json!({}))
}

pub fn handle_getmanifest() -> Result<Value, RpcError> {
    Ok(serde_json::json!({
        "options": [
            {
                "name": "pruning-interval",
                "type": "int",
                "default": 600,
                "description": "number of seconds to wait between pruning checks"
            }
        ],
        "rpcmethods": [],
        "subscriptions": [],
        "hooks": [],
        "features": {
            "node": "00000000",
            "init": "00000000",
            "invoice": "00000000"
        },
        "dynamic": true
    }))
}

pub fn handle_event(_method: &str, _params: &RpcParams) -> Result<(), String> {
    Ok(())
}

pub fn handle_req(sender: &Sender<InitInfo>, req: &RpcReq) -> Result<Option<Value>, RpcError> {
    match req {
        RpcReq {
            id: Some(_),
            method,
            params,
            ..
        } => match method.borrow() {
            "init" => Ok(Some(handle_init(sender, params)?)),
            "getmanifest" => Ok(Some(handle_getmanifest()?)),
            _ => Err(RpcError {
                code: 3.into(),
                message: Cow::Borrowed("unknown method"),
                data: Some(Value::String(method.to_string())),
            }),
        },
        RpcReq {
            id: None,
            method,
            params,
            ..
        } => {
            match handle_event(method, params) {
                Ok(_) => (),
                Err(e) => log::error!("RPC EVENT HANDLER ERROR: {}", e),
            };
            Ok(None)
        }
    }
}

pub fn run_rpc_handler(sender: Sender<InitInfo>) {
    let req_stream: StreamDeserializer<_, RpcReq> =
        StreamDeserializer::new(serde_json::de::IoRead::new(std::io::stdin()));
    // for request in stream
    for e_req in req_stream {
        match e_req {
            Ok(req) => {
                match (handle_req(&sender, &req).transpose(), req.id) {
                    (Some(res), Some(id)) => {
                        if let Err(e) = &res {
                            log::error!("RPC REQUEST HANDLER ERROR: {}", e);
                        }
                        serde_json::to_writer(
                            std::io::stdout(),
                            &RpcRes {
                                id,
                                jsonrpc: Default::default(),
                                result: res.into(),
                            },
                        )
                        .unwrap(); // if this fails, we cannot recover. Should never fail since coming from serde_json::Value
                        print!("\n\n");
                    }
                    _ => (),
                }
            }
            Err(e) => {
                serde_json::to_writer(
                    std::io::stdout(),
                    &RpcRes {
                        id: JsonRpcV2Id::Null,
                        jsonrpc: Default::default(),
                        result: RpcResult::Error(RpcError {
                            code: 1.into(),
                            message: Cow::Borrowed("deserialization error"),
                            data: Some(Value::String(format!("{}", e))),
                        }),
                    },
                )
                .unwrap(); // if this fails, we cannot recover. Should never fail since coming from serde_json::Value
                print!("\n\n");
            }
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LightningInit {
    options: LightningOptions,
    configuration: LightningConfig,
}

impl From<LightningInit> for InitInfo {
    fn from(li: LightningInit) -> Self {
        InitInfo {
            socket_path: li
                .configuration
                .lightning_dir
                .join(li.configuration.rpc_file),
            pruning_interval: li.options.pruning_interval,
        }
    }
}

fn default_pruning_interval() -> u64 {
    600
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LightningOptions {
    #[serde(default = "default_pruning_interval")]
    #[serde(deserialize_with = "deser_str_num")]
    pruning_interval: u64,
}

fn deser_str_num<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StrNum {
        Str(String),
        Num(u64),
    }
    let sn: StrNum = serde::Deserialize::deserialize(deserializer)?;
    Ok(match sn {
        StrNum::Str(s) => s.parse().map_err(|e| serde::de::Error::custom(e))?,
        StrNum::Num(n) => n,
    })
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LightningConfig {
    lightning_dir: PathBuf,
    rpc_file: String,
    startup: bool,
}
