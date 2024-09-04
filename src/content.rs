use core::str;

use anyhow::anyhow;
use bytes::Bytes;
use either::Either;
use log::debug;
use maplit::hashmap;
use pact_models::bodies::OptionalBody;
use pact_models::generators::{GenerateValue, Generator, NoopVariantMatcher, VariantMatcher};
use pact_models::prelude::ContentType;
use tonic::{Request, Response};
use querystring::{stringify, querify};

use crate::parser::{parse_field, parse_value};
use crate::proto;
use crate::utils::{from_value, to_value};

pub fn setup_form_urlencoded_contents(
  request: &Request<proto::ConfigureInteractionRequest>
) -> anyhow::Result<Response<proto::ConfigureInteractionResponse>> {
  match &request.get_ref().contents_config {
    Some(config) => {
      let mut fields = vec![];

      for (key, value) in &config.fields {
        if key.starts_with("field:") {
          let field = parse_field(&key)?;
          let result = parse_value(&value)?;
          debug!("Parsed field definition: {}, {:?}", field, result);
          fields.push((result, field))
        }
      }

      let field_values = fields.iter().map(|v| {
        let (md, name) = v;
        (name.as_str(), md.value.as_str())
      }).collect::<Vec<(&str, &str)>>();

      let mut markup = String::new();

      markup.push_str("```\n");
      markup.push_str(stringify(field_values.clone()).as_str());
      markup.push_str("```\n");

      let mut rules = hashmap!{};
      let mut generators = hashmap!{};
      for vals in fields.clone() {
        let (md, name) = vals;
        for rule in md.rules {
          if let Either::Left(rule) = rule {
            debug!("rule.values()={:?}", rule.values());
            rules.insert(format!("field:{}", name), proto::MatchingRules {
              rule: vec![
                proto::MatchingRule {
                  r#type: rule.name(),
                  values: Some(prost_types::Struct {
                    fields: rule.values().iter().map(|(key, val)| (key.to_string(), to_value(val))).collect()
                  })
                }
              ]
            });
          } else {
            return Ok(Response::new(proto::ConfigureInteractionResponse {
              error: format!("Expected a matching rule definition, but got an un-resolved reference {:?}", rule),
              .. proto::ConfigureInteractionResponse::default()
            }));
          }
        }

        if let Some(gen) = md.generator {
          generators.insert(format!("field:{}", name), proto::Generator {
            r#type: gen.name(),
            values: Some(prost_types::Struct {
              fields: gen.values().iter().map(|(key, val)| (key.to_string(), to_value(val))).collect()
            })
          });
        }
      }

      debug!("matching rules = {:?}", rules);
      debug!("generators = {:?}", generators);

      Ok(Response::new(proto::ConfigureInteractionResponse {
        interaction: vec![proto::InteractionResponse {
          contents: Some(proto::Body {
            content_type: "application/x-www-form-urlencoded".to_string(),
            content: Some(stringify(field_values).into_bytes()),
            content_type_hint: 0
          }),
          rules,
          generators,
          message_metadata: None,
          plugin_configuration: None,
          interaction_markup: markup,
          interaction_markup_type: 0,
          .. proto::InteractionResponse::default()
        }],
        .. proto::ConfigureInteractionResponse::default()
      }))
    }
    None => Err(anyhow!("No config provided to match/generate form urlencoded content"))
  }
}

pub fn generate_form_urlencoded_content(
  request: &Request<proto::GenerateContentRequest>
) -> anyhow::Result<OptionalBody> {
  let request = request.get_ref();

  let mut generators = hashmap! {};
  for (key, gen) in &request.generators {
    let field = parse_field(&key)?;
    let values = gen.values.as_ref().ok_or(anyhow!("Generator values were expected"))?.fields.iter().map(|(k, v)| {
      (k.clone(), from_value(v))
    }).collect();
    let generator = Generator::from_map(&gen.r#type, &values)
      .ok_or(anyhow!("Failed to build generator of type {}", gen.r#type))?;
    generators.insert(field, generator);
  };

  let context = hashmap! {};
  let variant_matcher = NoopVariantMatcher.boxed();

  let form_data = request.contents.as_ref().unwrap().content.as_ref().unwrap();
  let query = match str::from_utf8(form_data) {
      Ok(v) => v,
      Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
  };
  let field_values = querify(query);

  let mut query = vec![];
  for (name, value) in field_values.iter() {
    //let (name, value) = v;
    if let Some(generator) = generators.get(*name) {
      let value = generator.generate_value(&value.to_string(), &context, &variant_matcher)?;
      query.push((name, value))
    } else {
      query.push((name, value.to_string()))
    }
  }

  let generated = stringify(field_values).into_bytes();
  debug!("Generated contents has {} bytes", generated.len());
  let bytes = Bytes::from(generated);
  Ok(OptionalBody::Present(bytes, Some(ContentType::from("application/x-www-form-urlencoded")), None))
}
