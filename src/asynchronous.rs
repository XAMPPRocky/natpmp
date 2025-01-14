use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use async_trait::async_trait;

use crate::{
    convert_to, get_default_gateway, Error, GatewayResponse, MappingResponse, Protocol, Response,
    Result, NATPMP_MAX_ATTEMPS, NATPMP_PORT,
};

/// A wrapper trait for async udpsocket.
#[async_trait]
pub trait AsyncUdpSocket {
    async fn connect(&self, addr: &str) -> io::Result<()>;

    async fn send(&self, buf: &[u8]) -> io::Result<usize>;

    async fn recv(&self, buf: &mut [u8]) -> io::Result<usize>;
}

/// NAT-PMP async client
pub struct NatpmpAsync<S>
where
    S: AsyncUdpSocket,
{
    s: S,
    gateway: Ipv4Addr,
}

/// Create a NAT-PMP object with async udpsocket and gateway
pub fn new_natpmp_async_with<S>(s: S, gateway: Ipv4Addr) -> NatpmpAsync<S>
where
    S: AsyncUdpSocket,
{
    NatpmpAsync { s, gateway }
}

impl<S> NatpmpAsync<S>
where
    S: AsyncUdpSocket,
{
    /// NAT-PMP gateway address.
    pub fn gateway(&self) -> &Ipv4Addr {
        &self.gateway
    }

    pub async fn send_public_address_request(&mut self) -> Result<()> {
        let mut request = [0_u8; 2];
        let n = self
            .s
            .send(&request[..])
            .await
            .map_err(|e| Error::NATPMP_ERR_NETWORKFAILURE)?;
        if n != request.len() {
            return Err(Error::NATPMP_ERR_NETWORKFAILURE);
        }
        Ok(())
    }

    pub async fn send_port_mapping_request(
        &mut self,
        protocol: Protocol,
        private_port: u16,
        public_port: u16,
        lifetime: u32,
    ) -> Result<()> {
        let mut request = [0_u8; 12];
        request[1] = match protocol {
            Protocol::UDP => 1,
            _ => 2,
        };
        request[2] = 0; // reserved
        request[3] = 0; // reserved
                        // private port
        request[4] = (private_port >> 8 & 0xff) as u8;
        request[5] = (private_port & 0xff) as u8;
        // public port
        request[6] = (public_port >> 8 & 0xff) as u8;
        request[7] = (public_port & 0xff) as u8;
        // lifetime
        request[8] = ((lifetime >> 24) & 0xff) as u8;
        request[9] = ((lifetime >> 16) & 0xff) as u8;
        request[10] = ((lifetime >> 8) & 0xff) as u8;
        request[11] = (lifetime & 0xff) as u8;

        let n = self
            .s
            .send(&request[..])
            .await
            .map_err(|e| Error::NATPMP_ERR_NETWORKFAILURE)?;
        if n != request.len() {
            return Err(Error::NATPMP_ERR_NETWORKFAILURE);
        }
        Ok(())
    }

    pub async fn read_response_or_retry(&self) -> Result<Response> {
        let mut buf = [0_u8; 16];
        let mut retries = 0;
        while retries < NATPMP_MAX_ATTEMPS {
            match self.s.recv(&mut buf).await {
                Err(_) => retries += 1,
                Ok(n) => {
                    // version
                    if buf[0] != 0 {
                        return Err(Error::NATPMP_ERR_UNSUPPORTEDVERSION);
                    }
                    // opcode
                    if buf[1] < 128 || buf[1] > 130 {
                        return Err(Error::NATPMP_ERR_UNSUPPORTEDOPCODE);
                    }
                    // result code
                    let resultcode = u16::from_be(convert_to(&buf[2..4]));
                    // result
                    if resultcode != 0 {
                        return Err(match resultcode {
                            1 => Error::NATPMP_ERR_UNSUPPORTEDVERSION,
                            2 => Error::NATPMP_ERR_NOTAUTHORIZED,
                            3 => Error::NATPMP_ERR_NETWORKFAILURE,
                            4 => Error::NATPMP_ERR_OUTOFRESOURCES,
                            5 => Error::NATPMP_ERR_UNSUPPORTEDOPCODE,
                            _ => Error::NATPMP_ERR_UNDEFINEDERROR,
                        });
                    }
                    // epoch
                    let epoch = u32::from_be(convert_to(&buf[4..8]));
                    let rsp_type = buf[1] & 0x7f;
                    return Ok(match rsp_type {
                        0 => Response::Gateway(GatewayResponse {
                            epoch,
                            public_address: Ipv4Addr::from(u32::from_be(convert_to(&buf[8..12]))),
                        }),
                        _ => {
                            let private_port = u16::from_be(convert_to(&buf[8..10]));
                            let public_port = u16::from_be(convert_to(&buf[10..12]));
                            let lifetime = u32::from_be(convert_to(&buf[12..16]));
                            let lifetime = Duration::from_secs(u64::from(lifetime));
                            let m = MappingResponse {
                                epoch,
                                private_port,
                                public_port,
                                lifetime,
                            };
                            if rsp_type == 1 {
                                Response::UDP(m)
                            } else {
                                Response::TCP(m)
                            }
                        }
                    });
                }
            }
        }

        Err(Error::NATPMP_ERR_RECVFROM)
    }
}
