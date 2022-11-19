use crate::client::Client;
use crate::make::ClientFactory;
use crate::ton_config::load_ton_config;
use async_stream::try_stream;
use reqwest::Url;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use std::collections::HashSet;
use tokio_stream::Stream;
use tower::discover::Change;
use tower::limit::ConcurrencyLimit;
use tracing::{debug, info};
use crate::ton_config::Liteserver;
use tower::ServiceExt;
use tower::Service;

type DiscoverResult<K, S, E> = Result<Change<K, S>, E>;

pub struct DynamicServiceStream {
    changes: Pin<Box<dyn Stream<Item = Result<Change<String, ConcurrencyLimit<Client>>, anyhow::Error>> + Send>>,
}

impl DynamicServiceStream {
    pub(crate) fn new(url: Url, period: Duration) -> anyhow::Result<Self> {
        let mut interval = tokio::time::interval(period);
        let mut liteservers = HashSet::new();
        let mut factory = ClientFactory::default();

        // TODO[akostylev0] refac
        let stream = try_stream! {
            loop {
                interval.tick().await;

                info!("tick service discovery");
                let config = load_ton_config(url.clone()).await?;
                let liteserver_new: HashSet<Liteserver> = HashSet::from_iter(config.liteservers.iter().cloned());

                let liteservers_remove = liteservers.difference(&liteserver_new).collect::<Vec<&Liteserver>>();
                let liteservers_insert = liteserver_new.difference(&liteservers).collect::<Vec<&Liteserver>>();

                debug!("Discovered {} liteservers, remove {}, insert {}", liteserver_new.len(), liteservers_remove.len(), liteservers_insert.len());

                for ls in liteservers_remove {
                    debug!("remove {:?}", ls.id());
                    yield Change::Remove(ls.id());
                }

                for ls in liteservers_insert {
                    debug!("insert {:?}", ls.id());

                    if let Ok(client) = factory.ready().await?.call(config.with_liteserver(ls)).await {
                        yield Change::Insert(ls.id(), client);
                    }
                }

                liteservers = liteserver_new.clone();
            }
        };

        Ok(Self {
            changes: Box::pin(stream),
        })
    }
}

impl Stream for DynamicServiceStream {
    type Item = DiscoverResult<String, ConcurrencyLimit<Client>, anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.changes;
        match Pin::new(&mut *c).poll_next(cx) {
            Poll::Ready(Some(Ok(change))) => match change {
                Change::Insert(k, client) => Poll::Ready(Some(Ok(Change::Insert(
                    k,
                    client,
                )))),
                Change::Remove(k) => Poll::Ready(Some(Ok(Change::Remove(k)))),
            },
            _ => Poll::Pending
        }
    }
}
