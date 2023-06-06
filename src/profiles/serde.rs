use crate::profiles::Profile;
use serde::{de::Error, ser::SerializeMap};
use std::marker::PhantomData;

impl<T: serde::ser::Serialize> serde::ser::Serialize for Profile<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.name, &self.data)?;
        map.end()
    }
}

impl<'de, T: serde::de::Deserialize<'de>> serde::de::Deserialize<'de> for Profile<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_map(ProfileVisitor::default())
    }
}

struct ProfileVisitor<T> {
    phantom: PhantomData<T>,
}

impl<T> Default for ProfileVisitor<T> {
    fn default() -> Self {
        Self { phantom: PhantomData }
    }
}

impl<'de, T> serde::de::Visitor<'de> for ProfileVisitor<T>
where
    T: serde::de::Deserialize<'de>,
{
    type Value = Profile<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a map with single entry")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        match map.size_hint() {
            Some(1) => {
                let (name, data) = map.next_entry()?.unwrap();
                Ok(Profile { name, data })
            }
            None => match (map.next_entry()?, map.next_entry::<String, T>()?) {
                (Some((name, data)), None) => Ok(Profile { name, data }),
                (Some(_), Some(_)) => Err(A::Error::custom(format_args!(
                    "Too many entries. Expected {}",
                    &self as &dyn serde::de::Expected
                ))),
                (None, _) => Err(A::Error::invalid_length(0, &self)),
            },
            Some(len) => Err(A::Error::invalid_length(len, &self)),
        }
    }
}
