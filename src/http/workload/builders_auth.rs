use aws_credential_types::Credentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings, sign};
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use base64::Engine as _;
use reqwest::{RequestBuilder, Url};

use crate::{
    args::HttpMethod,
    error::{AppError, AppResult, HttpError},
};

use super::data::AuthConfig;

const fn http_method_str(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
    }
}

pub(super) fn apply_auth_headers(
    mut builder: RequestBuilder,
    method: HttpMethod,
    url: &Url,
    headers: &[(String, String)],
    body: &str,
    auth: &AuthConfig,
) -> AppResult<RequestBuilder> {
    match auth {
        AuthConfig::Basic { username, password } => {
            let token = format!("{}:{}", username, password);
            let encoded = base64::engine::general_purpose::STANDARD.encode(token.as_bytes());
            builder = builder.header("Authorization", format!("Basic {}", encoded));
            Ok(builder)
        }
        AuthConfig::SigV4 {
            access_key,
            secret_key,
            session_token,
            region,
            service,
        } => {
            let identity: Identity = Credentials::new(
                access_key,
                secret_key,
                session_token.clone(),
                None,
                "strest",
            )
            .into();
            let signing_settings = SigningSettings::default();
            let signing_params = v4::SigningParams::builder()
                .identity(&identity)
                .region(region)
                .name(service)
                .time(std::time::SystemTime::now())
                .settings(signing_settings)
                .build()
                .map_err(|err| {
                    AppError::http(HttpError::SigV4Params {
                        source: Box::new(err),
                    })
                })?
                .into();

            let method_str = http_method_str(method);
            let signable = SignableRequest::new(
                method_str,
                url.as_str(),
                headers.iter().map(|(k, v)| (k.as_str(), v.as_str())),
                SignableBody::Bytes(body.as_bytes()),
            )
            .map_err(|err| {
                AppError::http(HttpError::SigV4Request {
                    source: Box::new(err),
                })
            })?;

            let (instructions, _signature) = sign(signable, &signing_params)
                .map_err(|err| {
                    AppError::http(HttpError::SigV4Sign {
                        source: Box::new(err),
                    })
                })?
                .into_parts();

            let mut http_req = http::Request::builder()
                .method(method_str)
                .uri(url.as_str());
            for (key, value) in headers {
                http_req = http_req.header(key, value);
            }
            let mut http_req = http_req.body(()).map_err(|err| {
                AppError::http(HttpError::SigV4BuildSign {
                    source: Box::new(err),
                })
            })?;
            instructions.apply_to_request_http1x(&mut http_req);

            for (name, value) in http_req.headers() {
                builder = builder.header(name, value);
            }
            Ok(builder)
        }
    }
}
