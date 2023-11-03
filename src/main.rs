use anyhow::{anyhow, bail, Error, Result};
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new()
        .route("/:repo/SHA256SUMS", get(get_repo))
        .route("/:repo/:filename", get(get_blob))
        .fallback(get(catch_all));

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 5001));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

const REGISTRY: &str = "http://localhost:5000";

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let inner: serde_json::Value = serde_json::from_str(&self.0.to_string()).unwrap();
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": "internal server error", "inner": inner})),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

async fn catch_all(Path(rest): Path<String>) -> impl IntoResponse {
    println!("catch all: {}", rest);
    (StatusCode::NOT_FOUND, Json(json!({"message": "catch all"})))
}

async fn fetch_tag_list(name: &str) -> Result<Vec<String>> {
    let client = reqwest::Client::new();
    // query the OCI registry for tags
    let response = client
        .get(format!("{}/v2/{}/tags/list", REGISTRY, name))
        .send()
        .await?;
    if response.status() != StatusCode::OK {
        bail!("{}", response.text().await?);
    }
    let tags = response.json::<oci_spec::distribution::TagList>().await?;
    Ok(tags.tags().to_vec())
}

async fn fetch_digest(name: &str, reference: &str) -> Result<String> {
    let client = reqwest::Client::new();
    // query the OCI registry for tags
    let response = client
        .get(format!("{}/v2/{}/manifests/{}", REGISTRY, name, reference))
        .header("Accept", "application/vnd.oci.image.manifest.v1+json")
        .send()
        .await?;
    if response.status() != StatusCode::OK {
        bail!("{}", response.text().await?);
    }
    let tags = response.json::<oci_spec::image::ImageManifest>().await?;
    Ok(tags
        .layers()
        .first()
        .ok_or(anyhow!("no layers"))?
        .digest()
        .to_string())
}

async fn get_blob(
    Path((name, filename)): Path<(String, String)>,
) -> Result<axum::response::Redirect, AppError> {
    // split the filename into the tag and extension
    let tag = filename.strip_prefix(&format!("{}-", name)).ok_or(anyhow!(
        json!({"message": format!("failed to strip prefix {}- from {}", name, filename)})
    ))?;
    let tag = tag.strip_suffix(".raw").ok_or(anyhow!(
        json!({"message": format!("failed to strip suffix .raw from {}", tag)})
    ))?;
    let digest = fetch_digest(&name, &tag).await?;
    let url = format!("{}/v2/{}/blobs/{}", REGISTRY, name, digest);
    Ok(axum::response::Redirect::temporary(&url))
}

async fn get_repo(Path(name): Path<String>) -> Result<String, AppError> {
    // query the OCI registry for tags
    let tags = fetch_tag_list(&name).await?;
    let mut digests: Vec<(String, &str)> = Vec::new();
    for tag in tags.iter() {
        let digest = fetch_digest(&name, &tag).await?;
        digests.push((digest, tag));
    }
    println!("{:?}", tags);
    let mut response = String::new();
    const EXTENSION: &str = "raw";
    for digest in digests.iter() {
        let checksum = digest.0.strip_prefix("sha256:").ok_or(anyhow!(
            json!({"message": format!("invalid digest: {}", digest.0)})
        ))?;
        response.push_str(&format!(
            "{}  {}-{}.{}\n",
            checksum, name, digest.1, EXTENSION
        ));
    }

    Ok(response)
}
