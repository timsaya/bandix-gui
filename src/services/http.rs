use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 8686;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct Endpoint {
    host: String,
    port: u16,
}

impl Endpoint {
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim().trim_end_matches('/');
        if input.is_empty() {
            return Err("请输入服务地址".to_string());
        }
        if input.starts_with("https://") {
            return Err("Bandix 当前仅支持 http:// 地址".to_string());
        }

        let authority = input.strip_prefix("http://").unwrap_or(input);
        let authority = authority.split('/').next().unwrap_or(authority);
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
                let port = port.parse::<u16>().map_err(|_| "端口号无效".to_string())?;
                (host.to_string(), port)
            }
            _ => (authority.to_string(), DEFAULT_PORT),
        };

        if host.is_empty() || host.contains(char::is_whitespace) {
            return Err("服务地址无效".to_string());
        }
        Ok(Self { host, port })
    }

    pub fn resolve(&self) -> Result<SocketAddr, String> {
        (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|error| format!("无法解析地址：{error}"))?
            .find(|address| address.is_ipv4())
            .ok_or_else(|| "没有找到可用的 IPv4 地址".to_string())
    }

    pub fn display_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    pub fn connect(&self) -> Result<TcpStream, String> {
        TcpStream::connect_timeout(&self.resolve()?, CONNECT_TIMEOUT)
            .map_err(|error| format!("无法连接 {}:{}：{error}", self.host, self.port))
    }

    pub fn get(&self, path: &str) -> Result<HttpResponse, String> {
        self.request("GET", path, None)
    }

    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        let mut stream = self.connect()?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(|error| error.to_string())?;

        let body = body.unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.host,
            self.port,
            body.len(),
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| error.to_string())?;
        if !body.is_empty() {
            stream.write_all(body).map_err(|error| error.to_string())?;
        }

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| error.to_string())?;
        parse_response(&response)
    }

    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let response = self.get(path)?;
        if response.status != 200 {
            return Err(format!("HTTP {}", response.status));
        }

        let envelope: ApiEnvelope<T> = serde_json::from_slice(&response.body)
            .map_err(|error| format!("JSON 解析失败：{error}"))?;
        if envelope.status != "success" {
            return Err(envelope
                .message
                .unwrap_or_else(|| "Bandix 接口返回错误".to_string()));
        }
        envelope
            .data
            .ok_or_else(|| "Bandix 接口未返回 data".to_string())
    }

    pub fn send_json<T: Serialize>(
        &self,
        method: &str,
        path: &str,
        payload: &T,
    ) -> Result<(), String> {
        let body =
            serde_json::to_vec(payload).map_err(|error| format!("JSON 编码失败：{error}"))?;
        let response = self.request(method, path, Some(&body))?;
        if !(200..300).contains(&response.status) {
            return Err(format!("HTTP {}", response.status));
        }

        let envelope: ApiEnvelope<serde_json::Value> = serde_json::from_slice(&response.body)
            .map_err(|error| format!("JSON 解析失败：{error}"))?;
        if envelope.status != "success" {
            return Err(envelope
                .message
                .unwrap_or_else(|| "Bandix 接口返回错误".to_string()));
        }
        Ok(())
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    status: String,
    data: Option<T>,
    message: Option<String>,
}

fn parse_response(response: &[u8]) -> Result<HttpResponse, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "无效的 HTTP 响应".to_string())?;
    let head = String::from_utf8_lossy(&response[..header_end]);
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "无效的 HTTP 状态码".to_string())?;
    Ok(HttpResponse {
        status,
        body: response[header_end + 4..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_accepts_host_url_and_custom_port() {
        assert_eq!(
            Endpoint::parse("192.168.1.1").unwrap().display_url(),
            "http://192.168.1.1:8686"
        );
        assert_eq!(
            Endpoint::parse("http://router.local:9000/")
                .unwrap()
                .display_url(),
            "http://router.local:9000"
        );
    }
}
