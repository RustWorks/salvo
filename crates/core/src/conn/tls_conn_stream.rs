use std::future::Future;
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::{poll_fn, BoxFuture, FutureExt};
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::async_trait;
use crate::http::version::{self, Version, HttpConnection};

enum State<S> {
    Handshaking(BoxFuture<'static, IoResult<S>>),
    Ready(S),
    Error(IoError),
}

/// A handshake stream for tls.
pub struct TlsConnStream<S> {
    state: State<S>,
}

impl<S> TlsConnStream<S> {
    pub(crate) fn new<F>(handshake: F) -> Self
    where
        F: Future<Output = IoResult<S>> + Send + 'static,
    {
        Self {
            state: State::Handshaking(handshake.boxed()),
        }
    }
}

#[async_trait]
impl<S> HttpConnection for TlsConnStream<S>
where
    S: HttpConnection + Unpin + Send + 'static,
{
    async fn http_version(&mut self) -> Option<Version> {
        let mut fut = None;
        match &mut self.state {
            State::Handshaking(fut) => match fut.await {
                Ok(s) => self.state = State::Ready(s),
                Err(e) => {
                    self.state = State::Error(e);
                    return None;
                }
            },
            State::Ready(s) => {}
            State::Error(ref e) => {
                return None;
            }
        }
        if let State::Ready(s) = &mut self.state {
            fut = Some(s.http_version());
        }
        poll_fn(move |cx| fut.as_mut().map(|f| f.as_mut().poll(cx)).unwrap_or(Poll::Pending)).await
    }
}

impl<S> AsyncRead for TlsConnStream<S>
where
    S: AsyncRead + Unpin + Send + 'static,
{
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<IoResult<()>> {
        let this = &mut *self;

        loop {
            match &mut this.state {
                State::Handshaking(fut) => match fut.poll_unpin(cx) {
                    Poll::Ready(Ok(s)) => this.state = State::Ready(s),
                    Poll::Ready(Err(e)) => {
                        this.state = State::Error(e);
                    }
                    Poll::Pending => return Poll::Pending,
                },
                State::Ready(stream) => return Pin::new(stream).poll_read(cx, buf),
                State::Error(e) => return Poll::Ready(Err(IoError::new(ErrorKind::InvalidData, e.to_string()))),
            }
        }
    }
}

impl<S> AsyncWrite for TlsConnStream<S>
where
    S: AsyncWrite + Unpin + Send + 'static,
{
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        let this = &mut *self;
        loop {
            match &mut this.state {
                State::Handshaking(fut) => match fut.poll_unpin(cx) {
                    Poll::Ready(Ok(s)) => this.state = State::Ready(s),
                    Poll::Ready(Err(e)) => {
                        this.state = State::Error(e);
                    }
                    Poll::Pending => return Poll::Pending,
                },
                State::Ready(stream) => return Pin::new(stream).poll_write(cx, buf),
                State::Error(e) => return Poll::Ready(Err(IoError::new(ErrorKind::InvalidData, e.to_string()))),
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let this = &mut *self;

        loop {
            match &mut this.state {
                State::Handshaking(fut) => match fut.poll_unpin(cx) {
                    Poll::Ready(Ok(s)) => this.state = State::Ready(s),
                    Poll::Ready(Err(e)) => {
                        this.state = State::Error(e);
                    }
                    Poll::Pending => return Poll::Pending,
                },
                State::Ready(stream) => return Pin::new(stream).poll_flush(cx),
                State::Error(e) => return Poll::Ready(Err(IoError::new(ErrorKind::InvalidData, e.to_string()))),
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let this = &mut *self;

        loop {
            match &mut this.state {
                State::Handshaking(fut) => match fut.poll_unpin(cx) {
                    Poll::Ready(Ok(s)) => this.state = State::Ready(s),
                    Poll::Ready(Err(e)) => {
                        this.state = State::Error(e);
                    }
                    Poll::Pending => return Poll::Pending,
                },
                State::Ready(stream) => return Pin::new(stream).poll_shutdown(cx),
                State::Error(e) => return Poll::Ready(Err(IoError::new(ErrorKind::InvalidData, e.to_string()))),
            }
        }
    }
}
