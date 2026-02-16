use anyhow::Result;
use reqwest::Client;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tokio::time::sleep;

use crate::models::HealthResult;

pub async fn health_check(host: &str, port: u16) -> Result<HealthResult> {
    let resolved_host = normalize_host(host);
    let mut last_tcp = HealthResult {
        ok: false,
        status: 0,
        url: format!("tcp://{resolved_host}:{port}"),
        body: "No probe yet".to_string(),
    };
    for _ in 0..8 {
        if let Some(result) = tcp_probe(&resolved_host, port) {
            if result.ok {
                return Ok(result);
            }
            last_tcp = result;
        }
        sleep(Duration::from_millis(450)).await;
    }

    let base = format!("http://{resolved_host}:{port}");
    let endpoints = ["/health", "/v1/health", "/status", "/"];
    let client = Client::builder().timeout(Duration::from_secs(4)).build()?;

    let mut last = HealthResult {
        ok: false,
        status: 0,
        url: base.clone(),
        body: String::new(),
    };

    for endpoint in endpoints {
        let url = format!("{base}{endpoint}");
        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_default()
                    .chars()
                    .take(240)
                    .collect::<String>();
                if (200..300).contains(&status) {
                    return Ok(HealthResult {
                        ok: true,
                        status,
                        url,
                        body,
                    });
                }
                last = HealthResult {
                    ok: false,
                    status,
                    url,
                    body,
                };
            }
            Err(err) => {
                last = HealthResult {
                    ok: false,
                    status: 0,
                    url,
                    body: err.to_string(),
                };
            }
        }
    }

    if last.status == 0 {
        Ok(last_tcp)
    } else {
        Ok(last)
    }
}

fn normalize_host(host: &str) -> String {
    host.trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string()
}

fn tcp_probe(host: &str, port: u16) -> Option<HealthResult> {
    let mut last_err = None;
    let addrs = (host, port).to_socket_addrs().ok()?;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(_) => {
                return Some(HealthResult {
                    ok: true,
                    status: 200,
                    url: format!("tcp://{host}:{port}"),
                    body: "TCP connect succeeded".to_string(),
                })
            }
            Err(err) => last_err = Some(err.to_string()),
        }
    }
    Some(HealthResult {
        ok: false,
        status: 0,
        url: format!("tcp://{host}:{port}"),
        body: last_err.unwrap_or_else(|| "TCP probe failed".to_string()),
    })
}
