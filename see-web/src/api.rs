use gloo_net::http::Request;
use serde::de::DeserializeOwned;

const BASE: &str = "/api";

pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{BASE}{path}");
    let resp = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

pub async fn post<T: DeserializeOwned>(path: &str, body: &serde_json::Value) -> Result<T, String> {
    let url = format!("{BASE}{path}");
    let resp = Request::post(&url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

pub async fn put<T: DeserializeOwned>(path: &str, body: &serde_json::Value) -> Result<T, String> {
    let url = format!("{BASE}{path}");
    let resp = Request::put(&url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn delete<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{BASE}{path}");
    let resp = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

pub async fn get_text(path: &str) -> Result<String, String> {
    let url = format!("{BASE}{path}");
    let resp = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}
