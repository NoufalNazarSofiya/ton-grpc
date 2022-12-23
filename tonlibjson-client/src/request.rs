use std::time::Duration;
use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Service, ServiceExt};

#[async_trait]
pub trait Requestable where Self : Serialize + Sized {
    type Response : DeserializeOwned;

    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }

    fn into_request(self) -> anyhow::Result<Request> {
        let timeout = self.timeout();

        Request::with_timeout(self, timeout)
    }

    async fn call<S : Service<Request, Response = Value, Error = anyhow::Error>>(self, client: &mut S) -> Result<Self::Response, anyhow::Error>
        where S : Send, S::Future : Send
    {
        let request = self.into_request();

        let json = client
            .ready()
            .await?
            .call(request?)
            .await?;

        let response = serde_json::from_value::<Self::Response>(json)?;

        Ok(response)
    }
}

pub type RequestId = Uuid;

#[derive(Serialize, Clone)]
pub struct Request {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(skip_serializing)]
    pub timeout: Duration,

    #[serde(flatten)]
    pub data: Value
}

#[derive(Deserialize, Debug)]
pub struct Response {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(flatten)]
    pub data: Value
}

impl Request {
    pub fn new<T: Serialize>(data: T) -> anyhow::Result<Self> {
        Self::with_timeout(data, Duration::from_secs(3))
    }

    pub fn with_timeout<T: Serialize>(data: T, timeout: Duration) -> anyhow::Result<Self> {
        Ok(Self {
            id: RequestId::new_v4(),
            timeout,
            data: serde_json::to_value(data)?
        })
    }

    pub fn with_new_id(&self) -> Self {
        Self {
            id: RequestId::new_v4(),
            timeout: self.timeout,
            data: self.data.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;
    use serde_json::json;
    use uuid::Uuid;
    use crate::request::Request;

    #[test]
    fn data_is_flatten() {
        let request = Request {
            id: Uuid::from_str("7431f198-7514-40ff-876c-3e8ee0a311ba").unwrap(),
            timeout: Duration::from_secs(3),
            data: json!({
                "data": "is flatten"
            })
        };

        assert_eq!(serde_json::to_string(&request).unwrap(), "{\"@extra\":\"7431f198-7514-40ff-876c-3e8ee0a311ba\",\"data\":\"is flatten\"}")
    }
}
