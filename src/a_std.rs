use std::net::{Ipv4Addr, SocketAddrV4};
use std::io;

use async_std::net::UdpSocket;
use async_trait::async_trait;

use super::*;
use crate::asynchronous::{new_natpmp_async_with, AsyncUdpSocket, NatpmpAsync};

#[async_trait]
impl AsyncUdpSocket for UdpSocket {
    async fn connect(&self, addr: &str) -> io::Result<()> {
        self.connect(addr).await
    }

    async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf).await
    }

    async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv(buf).await
    }
}

pub async fn new_async_std_natpmp() -> Result<NatpmpAsync<UdpSocket>> {
    let gateway = get_default_gateway()?;
    new_async_std_natpmp_with(gateway).await
}

pub async fn new_async_std_natpmp_with(gateway: Ipv4Addr) -> Result<NatpmpAsync<UdpSocket>> {
    let s = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| Error::NATPMP_ERR_SOCKETERROR)?;
    let gateway_sockaddr = SocketAddrV4::new(gateway, NATPMP_PORT);
    if s.connect(gateway_sockaddr).await.is_err() {
        return Err(Error::NATPMP_ERR_CONNECTERR);
    }
    let n = new_natpmp_async_with(s, gateway);
    Ok(n)
}
