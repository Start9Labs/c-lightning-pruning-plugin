use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use serde_json::Value;
use tokio::sync::RwLock;
use url::Host;

#[derive(Clone, Debug)]
pub struct BitcoinInfo {
    pub bitcoin_rpcuser: String,
    pub bitcoin_rpcpassword: String,
    pub bitcoin_rpcconnect: Host<String>,
    pub bitcoin_rpcport: Option<u16>,
}
impl<'de> serde::Deserialize<'de> for BitcoinInfo {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub struct BitcoinInfoSerDe {
            pub bitcoin_rpcuser: Option<String>,
            pub bitcoin_rpcpassword: Option<String>,
            pub bitcoin_rpcconnect: Option<String>,
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
                .map(|a| Host::parse(&a))
                .transpose()
                .map_err(serde::de::Error::custom)?
                .unwrap_or_else(|| Host::Ipv4([127, 0, 0, 1].into())),
            bitcoin_rpcport: info.bitcoin_rpcport,
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
pub enum Network {
    Regtest,
    Testnet,
    Bitcoin,
}
impl Network {
    pub fn default_port(&self) -> u16 {
        match self {
            &Network::Regtest => 18443,
            &Network::Testnet => 18332,
            &Network::Bitcoin => 8332,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigInfo {
    pub network: Network,
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
