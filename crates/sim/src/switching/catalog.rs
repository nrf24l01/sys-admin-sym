use crate::{NetworkDeviceCategory, NetworkDeviceModel, network_device_catalog};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// String-backed identity; the constants are compatibility aliases, not model registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchModel(Cow<'static, str>);
impl Default for SwitchModel {
    fn default() -> Self {
        Self(Cow::Owned(network_device_catalog().default_switch.clone()))
    }
}
impl SwitchModel {
    #[allow(non_upper_case_globals)]
    pub const Catalyst24T4G: Self = Self(Cow::Borrowed("c1000_24t_4g_l"));
    #[allow(non_upper_case_globals)]
    pub const Catalyst24T4X: Self = Self(Cow::Borrowed("c1000_24t_4x_l"));
    pub fn parse(id: &str) -> Result<Self, String> {
        if !network_device_catalog()
            .model(id)
            .is_some_and(|m| m.category == NetworkDeviceCategory::Switch)
        {
            return Err(format!("unknown switch model: {id}"));
        }
        Ok(Self(Cow::Owned(id.into())))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn spec(&self) -> &'static NetworkDeviceModel {
        network_device_catalog()
            .model(self.as_str())
            .expect("validated switch model identity")
    }
}
// Keep the legacy RON unit-variant representation while allowing any catalog ID.
impl Serialize for SwitchModel {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_unit_variant("SwitchModel", 0, self.spec().id.as_str())
    }
}
impl<'de> Deserialize<'de> for SwitchModel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Identity;
        impl<'de> serde::de::Visitor<'de> for Identity {
            type Value = SwitchModel;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a catalog switch model ID")
            }
            fn visit_enum<A: serde::de::EnumAccess<'de>>(
                self,
                a: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::VariantAccess;
                struct Token(String);
                impl<'de> Deserialize<'de> for Token {
                    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                        struct Name;
                        impl serde::de::Visitor<'_> for Name {
                            type Value = String;
                            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                                f.write_str("model identifier")
                            }
                            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<String, E> {
                                Ok(s.into())
                            }
                        }
                        d.deserialize_identifier(Name).map(Token)
                    }
                }
                let (Token(id), variant) = a.variant::<Token>()?;
                variant.unit_variant()?;
                SwitchModel::parse(&id).map_err(serde::de::Error::custom)
            }
        }
        d.deserialize_enum("SwitchModel", &[], Identity)
    }
}
pub type SwitchSpec = NetworkDeviceModel;
