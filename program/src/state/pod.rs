macro_rules! impl_from_number {
    ($struct_name:ident, $int_type:ty, $array_size:expr) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        #[repr(transparent)]
        pub struct $struct_name([u8; $array_size]);

        impl From<$int_type> for $struct_name {
            #[inline(always)]
            fn from(value: $int_type) -> Self {
                $struct_name(value.to_le_bytes())
            }
        }

        impl From<$struct_name> for $int_type {
            #[inline(always)]
            fn from(value: $struct_name) -> Self {
                <$int_type>::from_le_bytes(value.0)
            }
        }

        impl core::ops::Add for $struct_name {
            type Output = Self;

            #[inline(always)]
            fn add(self, other: Self) -> Self::Output {
                let self_value: $int_type = self.into();
                let other_value: $int_type = other.into();
                let result = self_value + other_value;
                result.into()
            }
        }
    };
}

impl_from_number!(PodU128, u128, 16);
impl_from_number!(PodU64, u64, 8);
impl_from_number!(PodU32, u32, 4);
impl_from_number!(PodU16, u16, 2);
impl_from_number!(PodI128, i128, 16);
impl_from_number!(PodI64, i64, 8);
impl_from_number!(PodI32, i32, 4);
impl_from_number!(PodI16, i16, 2);
impl_from_number!(PodF64, f64, 8);
