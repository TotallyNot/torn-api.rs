pub use torn_api_macros::IntoOwned;

pub trait IntoOwned {
    type Owned;

    fn into_owned(self) -> Self::Owned;
}

impl<T> IntoOwned for Option<T>
where
    T: IntoOwned,
{
    type Owned = Option<T::Owned>;

    fn into_owned(self) -> Self::Owned {
        self.map(IntoOwned::into_owned)
    }
}

impl<T> IntoOwned for Vec<T> where T: IntoOwned {
    type Owned = Vec<<T as IntoOwned>::Owned>;

    fn into_owned(self) -> Self::Owned {
        let mut owned = Vec::with_capacity(self.len());
        for elem in self {
            owned.push(elem.into_owned());
        }
        owned
    }
} 

impl<K, V> IntoOwned for std::collections::HashMap<K, V> where V: IntoOwned, K: Eq + std::hash::Hash {
    type Owned = std::collections::HashMap<K, <V as IntoOwned>::Owned>;

    fn into_owned(self) -> Self::Owned {
        self.into_iter().map(|(k, v)| (k, v.into_owned())).collect()
    }
}

impl<K, V> IntoOwned for std::collections::BTreeMap<K, V> where V: IntoOwned, K: Eq + Ord + std::hash::Hash  {
    type Owned = std::collections::BTreeMap<K, <V as IntoOwned>::Owned>;

    fn into_owned(self) -> Self::Owned {
        self.into_iter().map(|(k, v)| (k, v.into_owned())).collect()
    }
}

impl<Z> IntoOwned for chrono::DateTime<Z> where Z: chrono::TimeZone {
    type Owned = Self;

    fn into_owned(self) -> Self::Owned {
        self
    }
}

impl<'a> IntoOwned for &'a str {
    type Owned = String;

    fn into_owned(self) -> Self::Owned {
        self.to_owned()
    }
}

macro_rules! impl_ident {
    ($name:path) => {
        impl IntoOwned for $name {
            type Owned = $name;
            fn into_owned(self) -> Self::Owned {
                self
            }
        }
    };
}

impl_ident!(i64);
impl_ident!(i32);
impl_ident!(i16);
impl_ident!(i8);
impl_ident!(String);
