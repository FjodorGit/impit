use std::collections::HashMap;
use reqwest::Response;
use url::Url;
use async_recursion::async_recursion;

use crate::{http_headers::HttpHeaders, tls};
use super::Browser;

#[derive(Debug, Clone)]
pub enum ErrorType {
  UrlParseError,
  ProtocolError,
  RequestError,
  ResponseError,
}

struct RetcherConfig {
  browser: Option<Browser>,
  vanilla_fallback: bool,
}

/// Retcher is the main struct used to make (impersonated) requests.
/// 
/// It uses `reqwest::Client` to make requests and holds info about the impersonated browser.
pub struct Retcher {
  client: reqwest::Client,
  config: RetcherConfig,
}

impl Default for Retcher {
  fn default() -> Self {
    RetcherBuilder::default().build()
  }
}

#[derive(Debug, Clone, Copy)]
pub struct RetcherBuilder {
  browser: Option<Browser>,
  ignore_tls_errors: bool,
  vanilla_fallback: bool,
}

impl Default for RetcherBuilder {
  fn default() -> Self {
    RetcherBuilder {
      browser: None,
      ignore_tls_errors: false,
      vanilla_fallback: true,
    }
  }
}

impl RetcherBuilder {
  pub fn with_browser(&mut self, browser: Browser) -> &mut Self {
    self.browser = Some(browser);
    self
  }

  pub fn with_ignore_tls_errors(&mut self, ignore_tls_errors: bool) -> &mut Self {
    self.ignore_tls_errors = ignore_tls_errors;
    self
  }

  pub fn with_fallback_to_vanilla(&mut self, vanilla_fallback: bool) -> &mut Self {
    self.vanilla_fallback = vanilla_fallback;
    self
  }

  pub fn build(self) -> Retcher {
    Retcher::new(self)
  }
}

impl Into<RetcherConfig> for RetcherBuilder {
  fn into(self) -> RetcherConfig {
    RetcherConfig {
      browser: self.browser,
      vanilla_fallback: self.vanilla_fallback,
    }
  }
}

/// RequestOptions is a struct holding additional options for the fetch request.
#[derive(Debug, Clone)]
pub struct RequestOptions {
  /// A `HashMap` that holds custom HTTP headers. These are added to the default headers and should never overwrite them.
  pub headers: HashMap<String, String>
}

impl Default for RequestOptions {
  fn default() -> Self {
    RequestOptions {
      headers: HashMap::new(),
    }
  }
}

impl Retcher {
  /// Creates a new `Retcher` instance with the given `EngineOptions`.
  fn new(builder: RetcherBuilder) -> Self {
    let mut client = reqwest::Client::builder();
    let tls_config = tls::TlsConfig::builder()
      .with_browser(builder.browser)
      .build();
    
    client = client
      .danger_accept_invalid_certs(builder.ignore_tls_errors)
      .danger_accept_invalid_hostnames(builder.ignore_tls_errors)
      .use_preconfigured_tls(tls_config);

    Retcher { 
      client: client.build().unwrap(), 
      config: builder.into()
    }
  }

  fn parse_url(&self, url: String) -> Result<Url, ErrorType> {
    let url = Url::parse(&url);

    if url.is_err() {
      return Err(ErrorType::UrlParseError);
    }
    let url = url.unwrap();

    if url.host_str().is_none() {
      return Err(ErrorType::UrlParseError);
    }

    let protocol = url.scheme();

    return match protocol {
      "http" => Ok(url),
      "https" => Ok(url),
      _ => Err(ErrorType::ProtocolError),
    };
  }

  pub fn builder() -> RetcherBuilder {
    RetcherBuilder::default()
  }

  #[async_recursion]
  pub async fn get(&self, url: String, options: Option<RequestOptions>) -> Result<Response, ErrorType> {
    let parsed_url = self.parse_url(url.clone());

    if parsed_url.is_err() {
      return Err(parsed_url.err().unwrap());
    }

    let parsed_url = parsed_url.unwrap();

    let headers = HttpHeaders::get_builder()
      .with_browser(self.config.browser)
      .with_host(parsed_url.host_str().unwrap().to_string())
      .with_https(parsed_url.scheme() == "https")
      .with_custom_headers(options.clone().unwrap_or_default().headers)
      .build();

    let request = self.client.get(parsed_url)
      .headers(headers.into());

    let response: Result<Response, reqwest::Error> = request.send().await;

    if response.is_err() {
      if !self.config.vanilla_fallback || self.config.browser.is_none() { 
        return Err(ErrorType::RequestError)
      }
      
      println!("Debug: encountered an error while using the browser impersonation, retrying with vanilla reqwest
{:#?}", response.err().unwrap());
      return Retcher::default().get(url, options).await;
    }
    
    Ok(response.unwrap())
  }
}
