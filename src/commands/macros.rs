macro_rules! sortable_by_key {
    ($ty_name:ident, $val_type:ty, |$val:ident| $field:expr) => {
        impl ::core::cmp::PartialEq for $ty_name {
            fn eq(&self, other: &Self) -> bool {
                let lhs: &$val_type = match self {
                    $val => $field,
                };
                let rhs: &$val_type = match other {
                    $val => $field,
                };
                lhs == rhs
            }
        }

        // Technically this forms a total equality relationship
        impl ::core::cmp::Eq for $ty_name where $val_type: Eq {}

        impl ::core::cmp::PartialOrd for $ty_name {
            fn partial_cmp(&self, other: &Self) -> Option<::core::cmp::Ordering> {
                // Only compare names
                Some(self.cmp(other))
            }
        }

        impl ::core::cmp::Ord for $ty_name {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                // Technically forms a total ordering
                let lhs: &$val_type = match self {
                    $val => $field,
                };
                let rhs: &$val_type = match other {
                    $val => $field,
                };
                ::core::cmp::Ord::cmp(lhs, rhs)
            }
        }
    };
}
