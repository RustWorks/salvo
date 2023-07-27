//! TcpListener and it's implements.
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::sync::Arc;
use std::vec;

use tokio::net::{TcpListener as TokioTcpListener, TcpStream, ToSocketAddrs};

use crate::async_trait;
use crate::conn::{Holding, HttpBuilder};
use crate::http::uri::Scheme;
use crate::http::{HttpConnection, Version};
use crate::service::HyperHandler;

use super::{Accepted, Acceptor, Listener};

#[cfg(any(feature = "rustls", feature = "native-tls", feature = "openssl"))]
use crate::conn::IntoConfigStream;

#[cfg(feature = "rustls")]
use crate::conn::rustls::{RustlsConfig, RustlsListener};

#[cfg(feature = "native-tls")]
use crate::conn::native_tls::{NativeTlsConfig, NativeTlsListener};

#[cfg(feature = "openssl")]
use crate::conn::openssl::{OpensslConfig, OpensslListener};

#[cfg(feature = "acme")]
use crate::conn::acme::AcmeListener;

/// TcpListener
pub struct TcpListener<T> {
    local_addr: T,
}
impl<T: ToSocketAddrs + Send> TcpListener<T> {
    /// Bind to socket address.
    #[inline]
    pub fn new(local_addr: T) -> Self {
        TcpListener { local_addr }
    }

    cfg_feature! {
        #![feature = "rustls"]

        /// Creates a new `RustlsListener` from current `TcpListener`.
        #[inline]
        pub fn rustls<C>(self, config_stream: C) -> RustlsListener<C, Self>
        where
            C: IntoConfigStream<RustlsConfig> + Send + 'static,
        {
            RustlsListener::new(config_stream, self)
        }
    }

    cfg_feature! {
        #![feature = "native-tls"]

        /// Creates a new `NativeTlsListener` from current `TcpListener`.
        #[inline]
        pub fn native_tls<C>(self, config_stream: C) -> NativeTlsListener<C, Self>
        where
            C: IntoConfigStream<NativeTlsConfig> + Send + 'static,
        {
            NativeTlsListener::new(config_stream, self)
        }
    }

    cfg_feature! {
        #![feature = "openssl"]

        /// Creates a new `OpensslListener` from current `TcpListener`.
        #[inline]
        pub fn openssl<C>(self, config_stream: C) -> OpensslListener<C, Self>
        where
            C: IntoConfigStream<OpensslConfig> + Send + 'static,
        {
            OpensslListener::new(config_stream, self)
        }
    }
    cfg_feature! {
        #![feature = "acme"]

        /// Creates a new `AcmeListener` from current `TcpListener`.
        #[inline]
        pub fn acme(self) -> AcmeListener<Self>
        {
            AcmeListener::new( self)
        }
    }
}
#[async_trait]
impl<T> Listener for TcpListener<T>
where
    T: ToSocketAddrs + Send,
{
    type Acceptor = TcpAcceptor;

    async fn bind(self) -> Self::Acceptor {
        self.try_bind().await.unwrap()
    }

    async fn try_bind(self) -> IoResult<Self::Acceptor> {
        TokioTcpListener::bind(self.local_addr).await?.try_into()
    }
}
/// TcpAcceptor
pub struct TcpAcceptor {
    inner: TokioTcpListener,
    holdings: Vec<Holding>,
}

impl TryFrom<TokioTcpListener> for TcpAcceptor {
    type Error = IoError;
    fn try_from(inner: TokioTcpListener) -> Result<Self, Self::Error> {
        let holding = Holding {
            local_addr: inner.local_addr()?.into(),
            http_versions: vec![Version::HTTP_11],
            http_scheme: Scheme::HTTP,
        };

        Ok(TcpAcceptor {
            inner,
            holdings: vec![holding],
        })
    }
}

#[async_trait]
impl HttpConnection for TcpStream {
    async fn serve(self, handler: HyperHandler, builder: Arc<HttpBuilder>) -> IoResult<()> {
        builder
            .serve_connection(self, handler)
            .await
            .map_err(|e| IoError::new(ErrorKind::Other, e.to_string()))
    }
}

#[async_trait]
impl Acceptor for TcpAcceptor {
    type Conn = TcpStream;

    #[inline]
    fn holdings(&self) -> &[Holding] {
        &self.holdings
    }

    #[inline]
    async fn accept(&mut self) -> IoResult<Accepted<Self::Conn>> {
        self.inner.accept().await.map(move |(conn, remote_addr)| Accepted {
            conn,
            local_addr: self.holdings[0].local_addr.clone(),
            remote_addr: remote_addr.into(),
            http_version: Version::HTTP_11,
            http_scheme: Scheme::HTTP,
        })
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;

    #[tokio::test]
    async fn test_tcp_listener() {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 6878));
        let mut acceptor = TcpListener::new(addr).bind().await;
        let addr = acceptor.holdings()[0].local_addr.clone().into_std().unwrap();
        tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            stream.write_i32(150).await.unwrap();
        });

        let Accepted { mut conn, .. } = acceptor.accept().await.unwrap();
        assert_eq!(conn.read_i32().await.unwrap(), 150);
    }
}
