use anyhow::anyhow;
use logos::Logos;
use pact_models::matchingrules::expressions::{MatchingRuleDefinition, parse_matcher_def};
use prost_types::value::Kind;

#[derive(Logos, Debug, PartialEq)]
enum FieldToken {
  #[token("field")]
  Field,

  #[token(":")]
  Colon,

  #[regex("[a-zA-Z]+")]
  Text,

  #[error]
  #[regex(r"[ \t\n\f]+", logos::skip)]
  Error,
}

// field -> "field" : text
pub(crate) fn parse_field(s: &str) -> anyhow::Result<String> {
  let mut lex = FieldToken::lexer(s);
  let first = lex.next();
  if first == Some(FieldToken::Field) {
    let second = lex.next();
    if second == Some(FieldToken::Colon) {
      let third = lex.next();
      if let Some(FieldToken::Text) = third {
        Ok(lex.slice().to_string())
      } else {
        Err(anyhow!("'{}' is not a valid field definition, expected a text, got '{}'", s, lex.remainder()))
      }
    } else {
      Err(anyhow!("'{}' is not a valid field definition, expected ':', got '{}'", s, lex.remainder()))
    }
  } else {
    Err(anyhow!("'{}' is not a valid field definition, expected 'field', got '{}'", s, lex.remainder()))
  }
}

pub(crate) fn parse_value(v: &prost_types::Value) -> anyhow::Result<MatchingRuleDefinition> {
  if let Some(kind) = &v.kind {
    match kind {
      Kind::StringValue(s) => parse_matcher_def(&s),
      Kind::NullValue(_) => Err(anyhow!("Null is not a valid value definition value")),
      Kind::NumberValue(_) => Err(anyhow!("Number is not a valid value definition value")),
      Kind::BoolValue(_) => Err(anyhow!("Bool is not a valid value definition value")),
      Kind::StructValue(_) => Err(anyhow!("Struct is not a valid value definition value")),
      Kind::ListValue(_) => Err(anyhow!("List is not a valid value definition value")),
    }
  } else {
    Err(anyhow!("Not a valid value definition (missing value)"))
  }
}
