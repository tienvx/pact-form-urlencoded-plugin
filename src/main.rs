use core::pin::Pin;
use core::task::{Context, Poll};
use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::net::SocketAddr;

use env_logger::Env;
use futures::Stream;
use log::debug;
use maplit::hashmap;
use pact_matching::matchers::Matches;
use pact_models::matchingrules::{MatchingRule, RuleList, RuleLogic};
use pact_models::prelude::ContentType;
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tonic::{Response, transport::Server};
use uuid::Uuid;
use querystring::querify;

use crate::content::{generate_form_urlencoded_content, setup_form_urlencoded_contents};
use crate::proto::body::ContentTypeHint;
use crate::proto::catalogue_entry::EntryType;
use crate::proto::pact_plugin_server::{PactPlugin, PactPluginServer};
use crate::proto::to_object;

mod proto;
mod parser;
mod utils;
mod content;

#[derive(Debug, Default)]
pub struct FormUrlEncodedPactPlugin {}

#[tonic::async_trait]
impl PactPlugin for FormUrlEncodedPactPlugin {

  // Returns the catalogue entries for Form Url Encoded
  async fn init_plugin(
    &self,
    request: tonic::Request<proto::InitPluginRequest>,
  ) -> Result<tonic::Response<proto::InitPluginResponse>, tonic::Status> {
    let message = request.get_ref();
    debug!("Init request from {}/{}", message.implementation, message.version);
    Ok(Response::new(proto::InitPluginResponse {
      catalogue: vec![
        proto::CatalogueEntry {
          r#type: EntryType::ContentMatcher as i32,
          key: "form-urlencoded".to_string(),
          values: hashmap! {
            "content-types".to_string() => "application/x-www-form-urlencoded".to_string()
          }
        },
        proto::CatalogueEntry {
          r#type: EntryType::ContentGenerator as i32,
          key: "form-urlencoded".to_string(),
          values: hashmap! {
            "content-types".to_string() => "application/x-www-form-urlencoded".to_string()
          }
        }
      ]
    }))
  }

  // Not used
  async fn update_catalogue(
    &self,
    _request: tonic::Request<proto::Catalogue>,
  ) -> Result<tonic::Response<()>, tonic::Status> {
    debug!("Update catalogue request, ignoring");
    Ok(Response::new(()))
  }

  // Request to compare the Form Url Encoded contents
  async fn compare_contents(
    &self,
    request: tonic::Request<proto::CompareContentsRequest>,
  ) -> Result<tonic::Response<proto::CompareContentsResponse>, tonic::Status> {
    let request = request.get_ref();
    debug!("compare_contents request - {:?}", request);

    match (request.expected.as_ref(), request.actual.as_ref()) {
      (Some(expected), Some(actual)) => {
        let expected_query = std::str::from_utf8(expected.content.as_ref().unwrap())
          .map_err(|err| tonic::Status::aborted(format!("Expected content is invalid UTF-8 string: {}", err)))?;
        let expected_query_params = querify(expected_query);
        let expected: HashMap<_, _> = expected_query_params.into_iter().map(|data| data).collect();
        let actual_query = std::str::from_utf8(actual.content.as_ref().unwrap())
          .map_err(|err| tonic::Status::aborted(format!("Actual content is invalid UTF-8 string: {}", err)))?;
        let actual_query_params = querify(actual_query);
        let actual: HashMap<_, _> = actual_query_params.into_iter().map(|data| data).collect();

        let rules = request.rules.iter()
          .map(|(key, rules)| {
            let rules = rules.rule.iter().fold(RuleList::empty(RuleLogic::And), |mut list, rule| {
              match to_object(&rule.values.as_ref().unwrap()) {
                Value::Object(mut map) => {
                  map.insert("match".to_string(), Value::String(rule.r#type.clone()));
                  debug!("Creating matching rule with {:?}", map);
                  list.add_rule(&MatchingRule::from_json(&Value::Object(map)).unwrap());
                }
                _ => {}
              }
              list
            });
            (key.clone(), rules)
          }).collect();
        compare_contents(expected, actual, request.allow_unexpected_keys, rules)
          .map_err(|err| tonic::Status::aborted(format!("Failed to compare Form Url Encoded contents: {}", err)))
      }
      (None, Some(actual)) => {
        let contents = actual.content.as_ref().unwrap();
        Ok(Response::new(proto::CompareContentsResponse {
          error: String::default(),
          type_mismatch: None,
          results: hashmap! {
            String::default() => proto::ContentMismatches {
              mismatches: vec![
                proto::ContentMismatch {
                  expected: None,
                  actual: Some(contents.clone()),
                  mismatch: format!("Expected no Form Url Encoded content, but got {} bytes", contents.len()),
                  path: "".to_string(),
                  diff: "".to_string()
                }
              ]
            }
          }
        }))
      }
      (Some(expected), None) => {
        let contents = expected.content.as_ref().unwrap();
        Ok(Response::new(proto::CompareContentsResponse {
          error: String::default(),
          type_mismatch: None,
          results: hashmap! {
            String::default() => proto::ContentMismatches {
              mismatches: vec![
                proto::ContentMismatch {
                  expected: Some(contents.clone()),
                  actual: None,
                  mismatch: format!("Expected Form Url Encoded content, but did not get any"),
                  path: "".to_string(),
                  diff: "".to_string()
                }
              ]
            }
          }
        }))
      }
      (None, None) => {
        Ok(Response::new(proto::CompareContentsResponse {
          error: String::default(),
          type_mismatch: None,
          results: hashmap!{}
        }))
      }
    }
  }

  // Request to configure the interaction with Form Url Encoded contents
  // Example definition we should receive:
  // "field:name", "matching(type,'Name')",
  // "field:age", "matching(number,100)",
  // "field:dob", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
  async fn configure_interaction(
    &self,
    request: tonic::Request<proto::ConfigureInteractionRequest>,
  ) -> Result<tonic::Response<proto::ConfigureInteractionResponse>, tonic::Status> {
    debug!("Received configure_contents request for '{}'", request.get_ref().content_type);
    setup_form_urlencoded_contents(&request)
      .map_err(|err| tonic::Status::aborted(format!("Invalid column definition: {}", err)))
  }

  // Request to generate Form Url Encoded contents
  async fn generate_content(
    &self,
    request: tonic::Request<proto::GenerateContentRequest>,
  ) -> Result<tonic::Response<proto::GenerateContentResponse>, tonic::Status> {
    debug!("Received generate_content request");
    generate_form_urlencoded_content(&request)
      .map(|contents| {
        debug!("Generated contents: {}", contents);
        Response::new(proto::GenerateContentResponse {
          contents: Some(proto::Body {
            content_type: contents.content_type().unwrap_or(ContentType::from("application/x-www-form-urlencoded")).to_string(),
            content: Some(contents.value().unwrap().to_vec()),
            content_type_hint: ContentTypeHint::Default as i32
          })
        })
      })
      .map_err(|err| tonic::Status::aborted(format!("Failed to generate Form Url Encoded contents: {}", err)))
  }
}

fn compare_contents(
  expected: HashMap<&str, &str>,
  actual: HashMap<&str, &str>,
  allow_unexpected_keys: bool,
  rules: HashMap<String, RuleList>
) -> anyhow::Result<tonic::Response<proto::CompareContentsResponse>> {
  debug!("Comparing contents using allow_unexpected_keys ({}) and rules ({:?})", allow_unexpected_keys, rules);

  let mut results = vec![];

  for (expected_name, expected_value) in expected.iter() {
    if !actual.contains_key(expected_name) {
      results.push(proto::ContentMismatch {
        expected: Some(expected_name.as_bytes().to_vec()),
        actual: None,
        mismatch: format!("Expected field '{}', but was missing", expected_name),
        path: String::default(),
        diff: String::default()
      });
    } else {
      compare_field(expected_name, expected_value, actual.get(expected_name).unwrap(), &rules, &mut results);
    }
  }

  if !allow_unexpected_keys {
    for (actual_name, _) in actual.iter() {
      if !expected.contains_key(actual_name) {
        results.push(proto::ContentMismatch {
          expected: None,
          actual: Some(actual_name.as_bytes().to_vec()),
          mismatch: format!("Unexpected field '{}', but was not allowed", actual_name),
          path: String::default(),
          diff: String::default()
        });
      }
    }
  }

  Ok(Response::new(proto::CompareContentsResponse {
    error: String::default(),
    type_mismatch: None,
    results: hashmap! {
      String::default() => proto::ContentMismatches {
        mismatches: results
      }
    }
  }))
}

fn compare_field(
  name: &str,
  expected_value: &str,
  actual_value: &str,
  rules: &HashMap<String, RuleList>,
  results: &mut Vec<proto::ContentMismatch>) {
  let path = format!("field:{}", name);

  if let Some(rules) = rules.get(&path) {
    for rule in &rules.rules {
      if let Err(err) = expected_value.matches_with(actual_value, rule, false) {
        results.push(proto::ContentMismatch {
          expected: Some(expected_value.as_bytes().to_vec()),
          actual: Some(actual_value.as_bytes().to_vec()),
          mismatch: err.to_string(),
          path: path.clone(),
          diff: String::default()
        });
      }
    }
  } else if actual_value != expected_value {
    results.push(proto::ContentMismatch {
      expected: Some(expected_value.as_bytes().to_vec()),
      actual: Some(actual_value.as_bytes().to_vec()),
      mismatch: format!("Expected field {} value to equal '{}', but got '{}'", name, expected_value, actual_value),
      path,
      diff: String::default()
    });
  }
}

struct TcpIncoming {
  inner: TcpListener
}

impl Stream for TcpIncoming {
  type Item = Result<TcpStream, std::io::Error>;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    Pin::new(&mut self.inner).poll_accept(cx)
      .map_ok(|(stream, _)| stream).map(|v| Some(v))
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let env = Env::new().filter("LOG_LEVEL");
  env_logger::init_from_env(env);

  let addr: SocketAddr = "0.0.0.0:0".parse()?;
  let listener = TcpListener::bind(addr).await?;
  let address = listener.local_addr()?;

  let server_key = Uuid::new_v4().to_string();
  println!("{{\"port\":{}, \"serverKey\":\"{}\"}}", address.port(), server_key);
  let _ = io::stdout().flush();

  let plugin = FormUrlEncodedPactPlugin::default();
  Server::builder()
    .add_service(PactPluginServer::new(plugin))
    .serve_with_incoming(TcpIncoming { inner: listener }).await?;

  Ok(())
}
