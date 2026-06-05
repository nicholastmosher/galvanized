pub mod subspace_id {
    use serde::{Deserializer, Serializer};
    use serde_with::{DeserializeAs, SerializeAs};
    use willow25::entry::SubspaceId;

    pub struct SubspaceIdDef;

    impl SerializeAs<SubspaceId> for SubspaceIdDef {
        fn serialize_as<S>(value: &SubspaceId, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let bytes = value.as_bytes();
            serde_bytes::serialize(bytes, serializer)
        }
    }

    impl<'de> DeserializeAs<'de, SubspaceId> for SubspaceIdDef {
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
}

pub mod namespace_id {
    use serde::{Deserializer, Serializer};
    use serde_with::{DeserializeAs, SerializeAs};
    use willow25::entry::NamespaceId;

    pub struct NamespaceIdDef;

    impl SerializeAs<NamespaceId> for NamespaceIdDef {
        fn serialize_as<S>(value: &NamespaceId, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let bytes = value.as_bytes();
            serde_bytes::serialize(bytes, serializer)
        }
    }

    impl<'de> DeserializeAs<'de, NamespaceId> for NamespaceIdDef {
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
}
