use std::borrow::Cow;

use serde_json::Value;

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum JsonRpcV2Id {
    Num(serde_json::Number),
    Str(String),
    Null,
}

#[derive(Clone, Debug)]
pub struct JsonRpcV2;
impl Default for JsonRpcV2 {
    fn default() -> Self {
        JsonRpcV2
    }
}
impl serde::Serialize for JsonRpcV2 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}
impl<'de> serde::Deserialize<'de> for JsonRpcV2 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let version: String = serde::Deserialize::deserialize(deserializer)?;
        match version.as_str() {
            "2.0" => (),
            a => {
                return Err(serde::de::Error::custom(format!(
                    "invalid RPC version: {}",
                    a
                )))
            }
        }
        Ok(JsonRpcV2)
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum RpcParams {
    ByPosition(Vec<Value>),
    ByName(serde_json::Map<String, Value>),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RpcReq {
    #[serde(default, deserialize_with = "deserialize_some")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JsonRpcV2Id>,
    #[serde(default)]
    pub jsonrpc: JsonRpcV2,
    pub method: Cow<'static, str>,
    pub params: RpcParams,
}
impl AsRef<RpcReq> for RpcReq {
    fn as_ref(&self) -> &RpcReq {
        &self
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RpcRes {
    pub id: JsonRpcV2Id,
    pub jsonrpc: JsonRpcV2,
    #[serde(flatten)]
    pub result: RpcResult,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RpcResult {
    Result(Value),
    Error(RpcError),
}
impl From<RpcResult> for Result<Value, RpcError> {
    fn from(r: RpcResult) -> Self {
        match r {
            RpcResult::Result(a) => Ok(a),
            RpcResult::Error(e) => Err(e),
        }
    }
}
impl From<Result<Value, RpcError>> for RpcResult {
    fn from(r: Result<Value, RpcError>) -> Self {
        match r {
            Ok(a) => RpcResult::Result(a),
            Err(e) => RpcResult::Error(e),
        }
    }
}

pub trait IntoRpcResult<T> {
    fn with_info<N: Into<serde_json::Number>>(
        self,
        code: N,
        msg: &'static str,
    ) -> Result<T, RpcError>;
}
impl<T, E> IntoRpcResult<T> for Result<T, E>
where
    E: serde::Serialize,
{
    fn with_info<N: Into<serde_json::Number>>(
        self,
        code: N,
        msg: &'static str,
    ) -> Result<T, RpcError> {
        match self {
            Ok(a) => Ok(a),
            Err(e) => Err(RpcError {
                code: code.into(),
                message: Cow::Borrowed(msg),
                data: Some(
                    serde_json::to_value(e)
                        .map_err(|e| format!("{}", e))
                        .with_info(2, "serialization error")?,
                ),
            }),
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RpcError {
    pub code: serde_json::Number,
    pub message: Cow<'static, str>,
    #[serde(
        default,
        deserialize_with = "deserialize_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub data: Option<Value>,
}
impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(data) = &self.data {
            write!(f, ": {}", data)?;
        }
        Ok(())
    }
}

pub async fn make_socket_req(
    socket: &mut tokio::net::UnixStream,
    req: RpcReq,
) -> Result<RpcRes, failure::Error> {
    use tokio::io::AsyncWriteExt;
    use tokio::stream::StreamExt;

    socket.write_all(&serde_json::to_vec(&req)?).await?;
    let res = crate::async_io::RpcResponseStream::new(socket)
        .next()
        .await
        .ok_or_else(|| {
            tokio::io::Error::new(tokio::io::ErrorKind::UnexpectedEof, "socket closed")
        })??;
    let res = serde_json::from_slice(&res)?;

    Ok(res)
}
