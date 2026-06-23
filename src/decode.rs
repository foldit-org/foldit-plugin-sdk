//! Proto-to-native conversion for the protocol types.

use std::collections::HashMap;

use crate::protocol::{DispatchContext, ParamValue, ResidueRef};
use crate::proto::plugin as proto;

/// Decode a `proto::DispatchContext` into the native [`DispatchContext`].
/// A `None` input yields the default (no focus, empty selection).
// The wire carries entity ids as u64, but molex::EntityId is u32. These ids
// are host-minted and fit u32 by construction, so the narrowing is sound.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn dispatch_context_from_proto(
    p: Option<proto::DispatchContext>,
) -> DispatchContext {
    let to_native = |refs: Vec<proto::ResidueRef>| -> Vec<ResidueRef> {
        refs.into_iter()
            .map(|r| ResidueRef {
                entity_id: molex::EntityId::from_raw(r.entity_id as u32),
                residue_index: r.residue_index,
            })
            .collect()
    };
    match p {
        Some(p) => DispatchContext {
            focused_entity_id: p
                .focused_entity_id
                .map(|raw| molex::EntityId::from_raw(raw as u32)),
            selection: to_native(p.selection),
            designable: to_native(p.designable),
        },
        None => DispatchContext::default(),
    }
}

/// Decode a proto param map into native [`ParamValue`]s. Entries whose
/// `value` oneof is unset are dropped.
#[must_use]
pub fn params_from_proto<S: std::hash::BuildHasher + Default>(
    p: HashMap<String, proto::ParamValue, S>,
) -> HashMap<String, ParamValue, S> {
    p.into_iter()
        .filter_map(|(k, v)| {
            let value = v.value?;
            let native = match value {
                proto::param_value::Value::IntValue(i) => ParamValue::Int(i),
                proto::param_value::Value::FloatValue(f) => {
                    ParamValue::Float(f)
                }
                proto::param_value::Value::BoolValue(b) => ParamValue::Bool(b),
                proto::param_value::Value::StringValue(s) => {
                    ParamValue::String(s)
                }
                proto::param_value::Value::Vec3Value(v3) => {
                    ParamValue::Vec3([v3.x, v3.y, v3.z])
                }
            };
            Some((k, native))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_context_decodes_ids_and_refs() {
        let ctx = dispatch_context_from_proto(Some(proto::DispatchContext {
            focused_entity_id: Some(7),
            selection: vec![proto::ResidueRef {
                entity_id: 7,
                residue_index: 3,
            }],
            designable: vec![proto::ResidueRef {
                entity_id: 2,
                residue_index: 0,
            }],
        }));
        assert_eq!(ctx.focused_entity_id, Some(molex::EntityId::from_raw(7)));
        assert_eq!(ctx.selection.len(), 1);
        assert_eq!(ctx.selection[0].entity_id, molex::EntityId::from_raw(7));
        assert_eq!(ctx.selection[0].residue_index, 3);
        assert_eq!(ctx.designable[0].entity_id, molex::EntityId::from_raw(2));
    }

    #[test]
    fn dispatch_context_none_is_default() {
        let ctx = dispatch_context_from_proto(None);
        assert!(ctx.focused_entity_id.is_none());
        assert!(ctx.selection.is_empty());
        assert!(ctx.designable.is_empty());
    }

    #[test]
    fn params_decode_each_variant_and_drop_unset() {
        let mut p: HashMap<String, proto::ParamValue> = HashMap::new();
        let _ = p.insert(
            "i".to_owned(),
            proto::ParamValue {
                value: Some(proto::param_value::Value::IntValue(5)),
            },
        );
        let _ = p.insert(
            "f".to_owned(),
            proto::ParamValue {
                value: Some(proto::param_value::Value::FloatValue(1.5)),
            },
        );
        let _ = p.insert(
            "b".to_owned(),
            proto::ParamValue {
                value: Some(proto::param_value::Value::BoolValue(true)),
            },
        );
        let _ = p.insert(
            "s".to_owned(),
            proto::ParamValue {
                value: Some(proto::param_value::Value::StringValue(
                    "x".to_owned(),
                )),
            },
        );
        let _ = p.insert(
            "v".to_owned(),
            proto::ParamValue {
                value: Some(proto::param_value::Value::Vec3Value(
                    proto::Vec3 {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    },
                )),
            },
        );
        let _ = p.insert(
            "unset".to_owned(),
            proto::ParamValue { value: None },
        );

        let native = params_from_proto(p);
        assert_eq!(native.len(), 5);
        assert_eq!(native.get("i"), Some(&ParamValue::Int(5)));
        assert_eq!(native.get("f"), Some(&ParamValue::Float(1.5)));
        assert_eq!(native.get("b"), Some(&ParamValue::Bool(true)));
        assert_eq!(
            native.get("s"),
            Some(&ParamValue::String("x".to_owned()))
        );
        assert_eq!(
            native.get("v"),
            Some(&ParamValue::Vec3([1.0, 2.0, 3.0]))
        );
        assert!(!native.contains_key("unset"));
    }
}
