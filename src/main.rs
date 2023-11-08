use anyhow::{anyhow, bail, Context, Error, Result};
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get},
    Json, Router,
};
use oci_spec::image::{Descriptor};
use reqwest;

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
        let inner: serde_json::Value = serde_json::from_str(&self.0.to_string())
            .with_context(|| format!("deserializing: {}", self.0))
            .unwrap();
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

async fn fetch_descriptor(name: &str, reference: &str) -> Result<Descriptor> {
    let client = reqwest::Client::new();
    // query the OCI registry for tags
    let response = client
        .get(format!("{}/v2/{}/manifests/{}", REGISTRY, name, reference))
        .header(
            reqwest::header::ACCEPT,
            "application/vnd.oci.image.manifest.v1+json",
        )
        .header(
            reqwest::header::ACCEPT,
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()
        .await?;
    if response.status() != StatusCode::OK {
        bail!("{}", response.text().await?);
    }
    let tags = response
        .json::<oci_spec::image::ImageManifest>()
        .await
        .context("reading image manifest")?;
    Ok(tags.layers().first().ok_or(anyhow!("no layers"))?.clone())
}

async fn get_blob(
    Path((name, filename)): Path<(String, String)>,
) -> Result<axum::response::Redirect, AppError> {
    // split the filename into the tag and extension
    let tag = filename.strip_prefix(&format!("{}-", name)).ok_or(anyhow!(
        json!({"message": format!("failed to strip prefix {}- from {}", name, filename)})
    ))?;
    let tag = tag
        .strip_suffix(EXTENSION_TGZ)
        .or_else(|| tag.strip_suffix(EXTENSION_RAW))
        .ok_or(anyhow!(
            json!({"message": format!("failed to strip suffix from {}", tag)})
        ))?;
    let desc = fetch_descriptor(&name, &tag).await?;
    let url = format!("{}/v2/{}/blobs/{}", REGISTRY, name, desc.digest());
    Ok(axum::response::Redirect::temporary(&url))
}

const EXTENSION_RAW: &str = ".raw";
const EXTENSION_TGZ: &str = ".tar.gz";

async fn get_repo(Path(name): Path<String>) -> Result<String, AppError> {
    // query the OCI registry for tags
    let tags = fetch_tag_list(&name).await?;
    let mut blobs: Vec<(Descriptor, &str)> = Vec::new();
    for tag in tags.iter() {
        let digest = fetch_descriptor(&name, &tag).await?;
        blobs.push((digest, tag));
    }
    println!("{:?}", tags);
    let mut response = String::new();
    for blob in blobs.iter() {
        let desc = &blob.0;
        let digest = desc.digest();
        let checksum = digest.strip_prefix("sha256:").ok_or(anyhow!(
            json!({"message": format!("invalid digest: {}", digest)})
        ))?;
        let extension = match desc.media_type().to_string().as_str() {
            "application/vnd.docker.image.rootfs.diff.tar.gzip" => EXTENSION_TGZ,
            _ => EXTENSION_RAW,
        };
        response.push_str(&format!(
            "{}  {}-{}{}\n",
            checksum, name, &blob.1, extension
        ));
    }

    Ok(response)
}
