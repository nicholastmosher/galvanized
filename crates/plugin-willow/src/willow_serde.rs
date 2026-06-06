use serde::{Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use willow25::entry::{NamespaceId, NamespaceSecret, SubspaceId, SubspaceSecret};

pub struct SubspaceIdSerde;

impl SerializeAs<SubspaceId> for SubspaceIdSerde {
    fn serialize_as<S>(value: &SubspaceId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = value.as_bytes();
        serde_bytes::serialize(bytes, serializer)
    }
}

impl<'de> DeserializeAs<'de, SubspaceId> for SubspaceIdSerde {
    fn deserialize_as<D>(deserializer: D) -> Result<SubspaceId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        let bytes = <[u8; 32]>::try_from(bytes).map_err(|vec| {
            serde::de::Error::custom(format!(
                "deserializing SubspaceId, expected [u8; 32], found Vec<u8> with len={}",
                vec.len()
            ))
        })?;

        let subspace_id = SubspaceId::from_bytes(&bytes);
        Ok(subspace_id)
    }
}

pub struct SubspaceSecretSerde;

impl SerializeAs<SubspaceSecret> for SubspaceSecretSerde {
    fn serialize_as<S>(value: &SubspaceSecret, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = value.as_bytes();
        serde_bytes::serialize(bytes, serializer)
    }
}

impl<'de> DeserializeAs<'de, SubspaceSecret> for SubspaceSecretSerde {
    fn deserialize_as<D>(deserializer: D) -> Result<SubspaceSecret, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        let bytes = <[u8; 32]>::try_from(bytes).map_err(|vec| {
            serde::de::Error::custom(format!(
                "deserializing SubspaceSecret, expected [u8; 32], found Vec<u8> with len={}",
                vec.len()
            ))
        })?;

        let subspace_secret = SubspaceSecret::from_bytes(&bytes);
        Ok(subspace_secret)
    }
}

pub struct NamespaceIdSerde;

impl SerializeAs<NamespaceId> for NamespaceIdSerde {
    fn serialize_as<S>(value: &NamespaceId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = value.as_bytes();
        serde_bytes::serialize(bytes, serializer)
    }
}

impl<'de> DeserializeAs<'de, NamespaceId> for NamespaceIdSerde {
    fn deserialize_as<D>(deserializer: D) -> Result<NamespaceId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        let bytes = <[u8; 32]>::try_from(bytes).map_err(|vec| {
            serde::de::Error::custom(format!(
                "deserializing NamespaceId, expected [u8; 32], found Vec<u8> with len={}",
                vec.len()
            ))
        })?;

        let namespace_id = NamespaceId::from_bytes(&bytes);
        Ok(namespace_id)
    }
}

pub struct NamespaceSecretSerde;

impl SerializeAs<NamespaceSecret> for NamespaceSecretSerde {
    fn serialize_as<S>(value: &NamespaceSecret, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = value.as_bytes();
        serde_bytes::serialize(bytes, serializer)
    }
}

impl<'de> DeserializeAs<'de, NamespaceSecret> for NamespaceSecretSerde {
    fn deserialize_as<D>(deserializer: D) -> Result<NamespaceSecret, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        let bytes = <[u8; 32]>::try_from(bytes).map_err(|vec| {
            serde::de::Error::custom(format!(
                "deserializing NamespaceSecret, expected [u8; 32], found Vec<u8> with len={}",
                vec.len()
            ))
        })?;

        let namespace_secret = NamespaceSecret::from_bytes(&bytes);
        Ok(namespace_secret)
    }
}
