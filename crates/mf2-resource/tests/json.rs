//! Feature `serde`: the JSON shape, and that it survives a round trip.

#![cfg(feature = "serde")]

use mf2_resource::{Resource, parse, serialize};
use std::borrow::Cow;

const SRC: &str = "\
# About this file.
@locale en-US
---

chat-send = Send

# floating

@param $count - How many.
users-online =
  .input {$count :integer}
  .match $count
  * {{{$count} online}}

@do-not-translate
[brand]
name = Example
";

#[test]
fn the_shape_is_the_documented_one() {
    let (resource, diags) = parse(SRC);
    assert!(diags.is_empty());
    let json: serde_json::Value = serde_json::to_value(&resource).expect("serializes");

    assert_eq!(json["comment"], "About this file.");
    assert_eq!(json["meta"][0]["name"], "locale");
    assert_eq!(json["meta"][0]["value"], "en-US");

    let head = &json["sections"][0];
    assert_eq!(head["id"], serde_json::json!([]));
    assert_eq!(head["entries"][0]["id"], serde_json::json!(["chat-send"]));
    assert_eq!(head["entries"][0]["value"], "Send");
    // A comment an empty line cut off keeps its place.
    assert_eq!(head["detached"][0]["before"], 1);
    assert_eq!(head["detached"][0]["comment"], "floating");
    assert_eq!(
        head["entries"][1]["meta"][0],
        serde_json::json!({ "name": "param", "value": "$count - How many." })
    );

    let brand = &json["sections"][1];
    assert_eq!(brand["id"], serde_json::json!(["brand"]));
    // A property without a value has no `value` key at all.
    assert_eq!(
        brand["meta"][0],
        serde_json::json!({ "name": "do-not-translate" })
    );
    assert_eq!(brand["entries"][0]["value"], "Example");
}

#[test]
fn json_round_trips_through_the_model_and_the_file() {
    let (resource, _) = parse(SRC);
    let json = serde_json::to_string(&resource).expect("serializes");
    let back: Resource<'_, Cow<'_, str>> = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, resource);

    // And what came from JSON writes a file that reads as the same thing.
    let written = serialize(&back).expect("writable");
    let (again, diags) = parse(&written);
    assert!(diags.is_empty(), "{diags:?} in\n{written}");
    assert_eq!(again, resource);
}

#[test]
fn missing_optional_fields_are_accepted() {
    let json = r#"{"sections":[{"id":["a"],"entries":[{"id":["b"],"value":"x"}]}]}"#;
    let resource: Resource<'_, Cow<'_, str>> = serde_json::from_str(json).expect("deserializes");
    assert_eq!(resource.locale(), None);
    let entry = resource.iter().next().expect("one entry");
    assert_eq!(entry.id().to_string(), "a.b");
    assert_eq!(entry.entry.value, "x");
}
