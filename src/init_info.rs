use std::fmt::Display;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use reqwest::Url;
use serde_json::Value;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct StringLike<SL>(pub SL);
impl<'de, SL: FromStr<Err = E>, E: Display> serde::Deserialize<'de> for StringLike<SL> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s: String = serde::Deserialize::deserialize(deserializer)?;
        FromStr::from_str(&s)
            .map(StringLike)
            .map_err(|e| serde::de::Error::custom(e))
    }
}
impl<SL: Display> serde::Serialize for StringLike<SL> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{}", self.0))
    }
}

#[derive(Clone, Debug)]
pub struct BitcoinInfo {
    pub bitcoin_rpcuser: String,
    pub bitcoin_rpcpassword: String,
    pub bitcoin_rpcconnect: Url,
    pub bitcoin_rpcport: u16,
}
impl<'de> serde::Deserialize<'de> for BitcoinInfo {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub struct BitcoinInfoSerDe {
            pub bitcoin_rpcuser: Option<String>,
            pub bitcoin_rpcpassword: Option<String>,
            pub bitcoin_rpcconnect: Option<StringLike<Url>>,
            pub bitcoin_rpcport: Option<u16>,
        }

        let info: BitcoinInfoSerDe = serde::Deserialize::deserialize(deserializer)?;
        Ok(BitcoinInfo {
            bitcoin_rpcuser: info.bitcoin_rpcuser.unwrap_or_else(|| "bitcoin".to_owned()),
            bitcoin_rpcpassword: info
                .bitcoin_rpcpassword
                .unwrap_or_else(|| "local321".to_owned()),
            bitcoin_rpcconnect: info
                .bitcoin_rpcconnect
                .map(|a| a.0)
                .unwrap_or_else(|| Url::parse("127.0.0.1").unwrap()),
            bitcoin_rpcport: info.bitcoin_rpcport.unwrap_or(8332),
        })
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PluginInfo {
    pub path: PathBuf,
    pub name: String,
    #[serde(default)]
    pub options: Value,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigInfo {
    pub always_use_proxy: bool,
    pub rescan: u64,
    pub proxy: Option<SocketAddr>,
    pub plugins: Vec<PluginInfo>,
}

#[derive(Clone, Debug)]
pub struct InitInfo {
    pub socket_path: PathBuf,
    pub pruning_interval: u64,
}

#[derive(Clone, Debug)]
pub enum InitInfoState {
    Waiting(Receiver<InitInfo>),
    Resolved(Arc<InitInfo>),
}

#[derive(Clone, Debug)]
pub struct InitInfoArc {
    state: Arc<RwLock<InitInfoState>>,
}

impl InitInfoArc {
    pub fn new(r: Receiver<InitInfo>) -> Self {
        InitInfoArc {
            state: Arc::new(RwLock::new(InitInfoState::Waiting(r))),
        }
    }
    pub async fn wait_for_info(self) -> Arc<InitInfo> {
        loop {
            let guard = self.state.read().await;
            match &*guard {
                InitInfoState::Resolved(ref path) => return path.clone(),
                InitInfoState::Waiting(receiver) => match receiver.try_recv() {
                    Ok(ii) => {
                        let arc_ii = Arc::new(ii);
                        drop(guard); // turns out this is important
                        let mut guard = self.state.write().await;
                        *guard = InitInfoState::Resolved(arc_ii.clone());
                        return arc_ii;
                    }
                    Err(_) => (),
                },
            }
        }
    }
}
