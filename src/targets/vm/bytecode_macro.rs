#[macro_export]
macro_rules! __bytecode {
    (
        $v:vis $bytecode_name:ident as $opcode_tag_type:ty ;

        opcode constants in $opcode_constants:ident ;

        prelude = $prelude:ident;

        instructive = $instructive:ident;
        encode = $encode:ident;
        decode = $decode:ident;

        opcodes {
            $($opcode:ident $({
                $(
                    // $(#[address] $($opcode_field_is_address:lifetime)?)?
                    $opcode_field_name:ident : $opcode_field_type:ty,
                )+
            })?,)+
        }

        value = $value_name:ident as $value_tag_type:ty;

        value constants in $value_constants:ident ;

        values {
            $($value:ident $({
                $(
                    // $(#[address] $($value_field_is_address:lifetime)?)?
                    $value_field_name:ident : $value_field_type:ty,
                )+
            })?,)+
        }

        garbage_collected = $garbage_collected_name:ident;

        arena index = $arena_index_name:ident as $arena_index_type:ty;

        garbage_collected {
            $($garbage_collected:ident $({
                $(
                    // $(#[address] $($garbage_collected_field_is_address:lifetime)?)?
                    $garbage_collected_field_name:ident : $garbage_collected_field_type:ty,
                )+
            })?,)*
        }
    ) => {
        $crate::__bytecode! (
            @GIVE ME DOLLAR@

            $v $bytecode_name as $opcode_tag_type ;

            opcode constants in $opcode_constants ;

            prelude = $prelude;

            instructive = $instructive;
            encode = $encode;
            decode = $decode;

            opcodes {
                $($opcode $({
                    $($opcode_field_name : $opcode_field_type,)+
                })?,)+
            }

            value = $value_name as $value_tag_type;

            value constants in $value_constants ;

            values {
                $($value $({
                    $($value_field_name : $value_field_type,)+
                })?,)+
            }

            garbage_collected = $garbage_collected_name;

            arena index = $arena_index_name as $arena_index_type;

            garbage_collected {
                $($garbage_collected $({
                    $($garbage_collected_field_name : $garbage_collected_field_type,)+
                })?,)*
            }

            $
        );
    };

    (
        @GIVE ME DOLLAR@

        $v:vis $bytecode_name:ident as $opcode_tag_type:ty ;

        opcode constants in $opcode_constants:ident ;

        prelude = $prelude:ident;

        instructive = $instructive:ident;
        encode = $encode:ident;
        decode = $decode:ident;

        opcodes {
            $($opcode:ident $({
                $(
                    // $(#[address] $($opcode_field_is_address:lifetime)?)?
                    $opcode_field_name:ident : $opcode_field_type:ty,
                )+
            })?,)+
        }

        value = $value_name:ident as $value_tag_type:ty;

        value constants in $value_constants:ident ;

        values {
            $($value:ident $({
                $(
                    // $(#[address] $($value_field_is_address:lifetime)?)?
                    $value_field_name:ident : $value_field_type:ty,
                )+
            })?,)+
        }

        garbage_collected = $garbage_collected_name:ident;

        arena index = $arena_index_name:ident as $arena_index_type:ty;

        garbage_collected {
            $($garbage_collected:ident $({
                $(
                    // $(#[address] $($garbage_collected_field_is_address:lifetime)?)?
                    $garbage_collected_field_name:ident : $garbage_collected_field_type:ty,
                )+
            })?,)*
        }

        $dollar:tt
    ) => {
        $v mod $prelude {
            pub use super::{$instructive, $encode, $decode, $bytecode_name, $value_name, $arena_index_name, $garbage_collected_name, $opcode_constants, $value_constants, $($opcode,)* $($value,)* $($garbage_collected,)*};
        }

        #[allow(unused)]
        $v trait $instructive: Sized {
            fn ip_mut(&mut self) -> &mut usize;

            fn constants_mut(&mut self) -> &mut Vec<$arena_index_name>;

            fn arena_mut(&mut self) -> &mut Vec<$garbage_collected_name>;

            fn ip(&self) -> usize;

            fn constants(&self) -> &[$arena_index_name];

            fn arena(&self) -> &[$garbage_collected_name];
        }

        #[allow(unused)]
        $v trait $encode<T>: Sized
        where
            T: $instructive,
        {
            type Output;

            fn encode(&self, instructive: &mut T) -> Self::Output;
        }

        #[allow(unused)]
        $v trait $decode<T>: Sized
        where
            T: $instructive,
        {
            type Output;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)>;
        }

        #[derive(Debug, Clone)]
        $v enum $bytecode_name {
            $(
                $opcode ($opcode),
            )+
        }

        impl<T> $encode<T> for $bytecode_name
        where
            T: $instructive,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                match self {
                    $(
                        Self::$opcode(opcode) => [
                            $opcode_constants::$opcode.encode(instructive),
                            opcode.encode(instructive),
                        ].concat(),
                    )+
                }
            }
        }

        impl<T> $decode<T> for $bytecode_name
        where
            T: $instructive,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                match bytecode.get(..(std::mem::size_of::<$opcode_tag_type>())).and_then(|bytes| bytes.as_array::<{std::mem::size_of::<$opcode_tag_type>()}>()).and_then(|bytes| <$opcode_tag_type>::decode(bytes, instructive)) {
                    $(
                        Some(($opcode_constants::$opcode, opcode_tag_size)) => {
                            let bytecode = &bytecode[opcode_tag_size..];

                            $opcode::decode(bytecode, instructive).map(|(opcode, size)| (Self::$opcode(opcode), size + opcode_tag_size))
                        }
                    )+
                    _ => None,
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        $v enum $value_name {
            $(
                $value ($value),
            )+
            $arena_index_name($arena_index_name),
        }

        impl<T> $encode<T> for $value_name
        where
            T: $instructive,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                match self {
                    $(
                        Self::$value(value) => [
                            $value_constants::$value.encode(instructive),
                            value.encode(instructive),
                        ].concat(),
                    )+
                    Self::$arena_index_name(value) => instructive.arena_mut()
                        .get(usize::from(*value))
                        .cloned()
                        .map(|gc_value| gc_value.encode(instructive))
                        .unwrap_or_else(|| {
                            [
                                $value_constants::$arena_index_name.encode(instructive),
                                value.encode(instructive),
                            ].concat()
                        }),
                }
            }
        }

        impl<T> $decode<T> for $value_name
        where
            T: $instructive,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                match bytecode.get(..(std::mem::size_of::<$value_tag_type>())).and_then(|bytes| bytes.as_array::<{std::mem::size_of::<$value_tag_type>()}>()).and_then(|bytes| <$value_tag_type>::decode(bytes, instructive)) {
                    $(
                        Some(($value_constants::$value, value_tag_size)) => {
                            let bytecode = &bytecode[value_tag_size..];

                            $value::decode(bytecode, instructive).map(|(value, size)| (Self::$value(value), size + value_tag_size))
                        }
                    )+
                    $(
                        Some(($value_constants::$garbage_collected, _)) => {
                            $garbage_collected_name::decode(bytecode, instructive)
                        }
                    )+
                    _ => None,
                }
            }
        }

        #[derive(Debug, Clone)]
        $v enum $garbage_collected_name {
            $(
                $garbage_collected ($garbage_collected),
            )+
        }

        impl<T> $encode<T> for $garbage_collected_name
        where
            T: $instructive,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                match self {
                    $(
                        Self::$garbage_collected(garbage_collected) => [
                            $value_constants::$garbage_collected.encode(instructive),
                            garbage_collected.encode(instructive),
                        ].concat(),
                    )+
                }
            }
        }

        impl<T> $decode<T> for $garbage_collected_name
        where
            T: $instructive,
        {
            type Output = $value_name;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                match bytecode.get(..(std::mem::size_of::<$value_tag_type>())).and_then(|bytes| bytes.as_array::<{std::mem::size_of::<$value_tag_type>()}>()).and_then(|bytes| <$value_tag_type>::decode(bytes, instructive)) {
                    $(
                        Some(($value_constants::$garbage_collected, garbage_collected_tag_size)) => {
                            let bytecode = &bytecode[garbage_collected_tag_size..];

                            let (value, value_size) = $garbage_collected::decode(bytecode, instructive).map(|(garbage_collected, size)| (Self::$garbage_collected(garbage_collected), size + garbage_collected_tag_size))?;

                            instructive.arena_mut().push(value);

                            let value_index = $arena_index_name { index: <$arena_index_type>::try_from(instructive.arena_mut().len() - 1).unwrap_or_else(|_| panic!("arena index was not a valid {}", stringify!($arena_index_type))) };

                            instructive.constants_mut().push(value_index);

                            Some(($value_name::$arena_index_name(value_index), value_size))
                        }
                    )+
                    _ => None,
                }
            }
        }

        impl From<$arena_index_name> for $arena_index_type {
            fn from(value: $arena_index_name) -> Self {
                value.index
            }
        }

        impl From<$arena_index_name> for usize {
            fn from(value: $arena_index_name) -> Self {
                Self::try_from(value.index).unwrap_or_else(|_| panic!("number of values was not a valid usize"))
            }
        }

        impl From<$arena_index_type> for $arena_index_name {
            fn from(value: $arena_index_type) -> Self {
                Self { index: value }
            }
        }

        impl From<usize> for $arena_index_name {
            fn from(value: usize) -> Self {
                Self {
                    index: <$arena_index_type>::try_from(value)
                        .unwrap_or_else(|_| panic!("number of values may not exceed {}", <$arena_index_type>::MAX)),
                }
            }
        }

        macro_rules! encode_decode_number {
            (
                $dollar number_type:ty $dollar (,)?
            ) => {
                impl<T> $encode<T> for $dollar number_type
                where
                    T: $instructive,
                {
                    type Output = Vec<u8>;

                    fn encode(&self, _: &mut T) -> Self::Output {
                        self.to_le_bytes().to_vec()
                    }
                }

                impl<T> $decode<T> for $dollar number_type
                where
                    T: $instructive,
                {
                    type Output = Self;

                    fn decode(bytecode: &[u8], _: &mut T) -> Option<(Self::Output, usize)> {
                        let Some(value) = bytecode
                            .get(..std::mem::size_of::<$dollar number_type>())
                            .and_then(|bytes| bytes.as_array::<{ std::mem::size_of::<$dollar number_type>() }>())
                            .map(|bytes| <$dollar number_type>::from_le_bytes(*bytes))
                        else {
                            panic!("expected a {}", stringify!($dollar number_type));
                        };

                        Some((value, std::mem::size_of::<$dollar number_type>()))
                    }
                }
            };
        }

        encode_decode_number!(u8);
        encode_decode_number!(i8);
        encode_decode_number!(u16);
        encode_decode_number!(i16);
        encode_decode_number!(u32);
        encode_decode_number!(i32);
        encode_decode_number!(u64);
        encode_decode_number!(i64);
        encode_decode_number!(f32);
        encode_decode_number!(f64);

        impl<T, U> $encode<T> for Vec<U>
        where
            T: $instructive,
            U: $encode<T, Output = Vec<u8>>,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                [
                    u64::try_from(self.len())
                        .unwrap_or_else(|_| panic!("length of vec was not a valid u64"))
                        .to_le_bytes()
                        .to_vec(),
                    self.iter()
                        .fold(vec![], |mut accum, value| {
                            accum.push(value.encode(instructive));

                            accum
                        })
                        .concat(),
                ]
                .concat()
            }
        }

        impl<T, U> $decode<T> for Vec<U>
        where
            T: $instructive,
            U: $decode<T, Output = U>,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                let Some((length, length_size)) = u64::decode(bytecode, instructive) else {
                    panic!("length of vec was not a valid u64");
                };

                let mut total_size = length_size;

                let values = (0..length)
                    .map(|i| {
                        let Some((value, value_size)) = U::decode(&bytecode[total_size..], instructive)
                        else {
                            panic!("value {i} in vec was not valid");
                        };

                        total_size += value_size;

                        value
                    })
                    .collect::<Vec<_>>();

                Some((values, total_size))
            }
        }

        impl<T, U> $encode<T> for Box<[U]>
        where
            T: $instructive,
            U: $encode<T, Output = Vec<u8>>,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                [
                    u64::try_from(self.len())
                        .unwrap_or_else(|_| panic!("length of boxed slice was not a valid u64"))
                        .to_le_bytes()
                        .to_vec(),
                    self.iter()
                        .fold(vec![], |mut accum, value| {
                            accum.push(value.encode(instructive));

                            accum
                        })
                        .concat(),
                ]
                .concat()
            }
        }

        impl<T, U> $decode<T> for Box<[U]>
        where
            T: $instructive,
            U: $decode<T, Output = U>,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                let Some((length, length_size)) = u64::decode(bytecode, instructive) else {
                    panic!("length of boxed slice was not a valid u64");
                };

                let mut total_size = length_size;

                let values = (0..length)
                    .map(|i| {
                        let Some((value, value_size)) = U::decode(&bytecode[total_size..], instructive)
                        else {
                            panic!("value {i} in vec was not valid");
                        };

                        total_size += value_size;

                        value
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();

                Some((values, total_size))
            }
        }

        impl<T, U, const N: usize> $encode<T> for [U; N]
        where
            T: $instructive,
            U: $encode<T, Output = Vec<u8>>,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                [
                    self.iter()
                        .fold(vec![], |mut accum, value| {
                            accum.push(value.encode(instructive));

                            accum
                        })
                        .concat(),
                ]
                .concat()
            }
        }

        impl<T, U, const N: usize> $decode<T> for [U; N]
        where
            T: $instructive,
            U: $decode<T, Output = U>,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                let mut total_size = 0;

                let values = *<Box<[U; N]>>::try_from((0..N)
                    .map(|i| {
                        let Some((value, value_size)) = U::decode(&bytecode[total_size..], instructive)
                        else {
                            panic!("value {i} in array was not valid");
                        };

                        total_size += value_size;

                        value
                    })
                    .collect::<Vec<_>>())
                    .unwrap_or_else(|_| panic!("array of size {N} did not have the correct amount of values"));

                Some((values, total_size))
            }
        }

        impl<T> $encode<T> for &str
        where
            T: $instructive,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                [
                    u64::try_from(self.len()).map_or_else(
                        |_| panic!("string length must fit within {}", u64::MAX),
                        |length| length.encode(instructive),
                    ),
                    self.as_bytes().to_vec(),
                ]
                .concat()
            }
        }

        impl<T> $encode<T> for String
        where
            T: $instructive,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                self.as_str().encode(instructive)
            }
        }

        impl<T> $decode<T> for String
        where
            T: $instructive,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                let Some((length, length_size)) = bytecode
                    .get(..std::mem::size_of::<u64>())
                    .and_then(|bytes| bytes.as_array::<{ std::mem::size_of::<u64>() }>())
                    .and_then(|bytes| u64::decode(bytes, instructive))
                else {
                    panic!("string length was not a valid u64");
                };

                let bytecode = &bytecode[length_size..];

                let length = usize::try_from(length)
                    .unwrap_or_else(|_| panic!("string length was not a valid usize"));

                Some((
                    str::from_utf8(&bytecode[..length])
                        .unwrap_or_else(|_| panic!("string was not valid utf-8"))
                        .to_string(),
                    length + std::mem::size_of::<u64>(),
                ))
            }
        }

        impl<T> $encode<T> for bool
        where
            T: $instructive,
        {
            type Output = Vec<u8>;

            fn encode(&self, instructive: &mut T) -> Self::Output {
                u8::from(*self).encode(instructive)
            }
        }

        impl<T> $decode<T> for bool
        where
            T: $instructive,
        {
            type Output = Self;

            fn decode(bytecode: &[u8], _: &mut T) -> Option<(Self::Output, usize)> {
                Some((
                    Self::try_from(bytecode[0]).unwrap_or_else(|_| panic!("bool was not a valid bool")),
                    1,
                ))
            }
        }

        macro_rules! opcode {
            (
                @CONSTANTS $dollar constants:ident
                $dollar (#[$dollar attribute:meta])*
                $dollar opcode:ident { } -> { $dollar ($dollar result_fields:tt)* }
                [$dollar ($dollar field_name:ident : $dollar field_type:ty,)+]
                @BACKPATCH [$dollar ($dollar address_field_name:ident,)*]
            ) => {
                $dollar (#[$dollar attribute])*
                $v struct $dollar opcode {
                    $dollar ($dollar result_fields)+
                }

                impl<T> $encode<T> for $dollar opcode
                where
                    T: $instructive,
                {
                    type Output = Vec<u8>;

                    #[allow(clippy::unused_self)]
                    fn encode(&self, instructive: &mut T) -> Self::Output {
                        [
                            $dollar (self.$dollar field_name.encode(instructive).to_vec(),)+
                        ].concat()
                    }
                }

                impl<T> $decode<T> for $dollar opcode
                where
                    T: $instructive,
                {
                    type Output = Self;

                    #[allow(clippy::unnecessary_wraps)]
                    fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
                        let total_size = 0;

                        $dollar (
                            let Some(($dollar field_name, field_size)) = <$dollar field_type>::decode(&bytecode[total_size..], instructive)
                            else {
                                panic!("{}'s \"{}\" was not a valid {}", stringify!($dollar opcode), stringify!($dollar field_name), stringify!($dollar field_type));
                            };

                            #[allow(unused)]
                            let total_size = total_size + field_size;
                        )+

                        Some(($dollar opcode {
                            $dollar ($dollar field_name,)+
                        }, total_size))
                    }
                }
            };

            // (
            //     @CONSTANTS $dollar constants:ident
            //     $dollar (#[$dollar attribute:meta])*
            //     $dollar opcode:ident {
            //         #[address]
            //         $dollar field_name:ident : u32,
            //         $dollar ($dollar rest_of_fields:tt)*
            //     } -> { $dollar ($dollar result_fields:tt)* }
            //     [$dollar ($dollar original_field_name:ident : $dollar original_field_type:ty,)+]
            //     @BACKPATCH [$dollar ($dollar address_field_name:ident,)*]
            // ) => {
            //     opcode! {
            //         @CONSTANTS $dollar constants
            //         $dollar (#[$dollar attribute])*
            //         $dollar opcode { $dollar ($dollar rest_of_fields)* } -> {
            //             $dollar ($dollar result_fields)*
            //             pub $dollar field_name : u32,
            //         }
            //         [$dollar ($dollar original_field_name : $dollar original_field_type,)+]
            //         @BACKPATCH [$dollar ($dollar address_field_name,)* $dollar field_name,]
            //     }
            // };

            (
                @CONSTANTS $dollar constants:ident
                $dollar (#[$dollar attribute:meta])*
                $dollar opcode:ident {
                    $dollar field_name:ident : $dollar field_type:ty,
                    $dollar ($dollar rest_of_fields:tt)*
                } -> { $dollar ($dollar result_fields:tt)* }
                [$dollar ($dollar original_field_name:ident : $dollar original_field_type:ty,)+]
                @BACKPATCH [$dollar ($dollar address_field_name:ident,)*]
            ) => {
                opcode! {
                    @CONSTANTS $dollar constants
                    $dollar (#[$dollar attribute])*
                    $dollar opcode { $dollar ($dollar rest_of_fields)* } -> {
                        $dollar ($dollar result_fields)*
                        pub $dollar field_name : $dollar field_type,
                    }
                    [$dollar ($dollar original_field_name : $dollar original_field_type,)+]
                    @BACKPATCH [$dollar ($dollar address_field_name,)*]
                }
            };

            (
                @CONSTANTS $dollar constants:ident
                $dollar (#[$dollar attribute:meta])*
                $dollar opcode:ident {
                    $dollar (
                        // $dollar (#[address] $dollar ($dollar field_is_address:lifetime)?)?
                        $dollar field_name:ident : $dollar field_type:ty,
                    )+
                } -> { }
                [ ]
                @BACKPATCH []
            ) => {
                opcode! {
                    @CONSTANTS $dollar constants
                    $dollar (#[$dollar attribute])*
                    $dollar opcode {
                        $dollar (
                            // $dollar (#[address] $dollar ($dollar field_is_address:lifetime)?)?
                            $dollar field_name : $dollar field_type,
                        )+
                    } -> { }
                    [$dollar ($dollar field_name : $dollar field_type,)+]
                    @BACKPATCH []
                }
            };

            (
                @CONSTANTS $dollar constants:ident
                $dollar (#[$dollar attribute:meta])*
                $dollar opcode:ident
            ) => {
                $dollar (#[$dollar attribute])*
                $v struct $dollar opcode;

                impl<T> $encode<T> for $dollar opcode
                where
                    T: $instructive,
                {
                    type Output = Vec<u8>;

                    #[allow(clippy::unused_self)]
                    fn encode(&self, _: &mut T) -> Self::Output {
                        vec![]
                    }
                }

                impl<T> $decode<T> for $dollar opcode
                where
                    T: $instructive,
                {
                    type Output = Self;

                    fn decode(_: &[u8], _: &mut T) -> Option<(Self::Output, usize)> {
                        Some(($dollar opcode, 0))
                    }
                }
            };
        }

        macro_rules! enumerate {
            (
                @CONSTANTS $dollar constants:ident
                { } -> { $dollar previous_opcode:ident $dollar ($dollar result_opcodes:tt)* }
            ) => {
                #[allow(unused, non_upper_case_globals)]
                $v mod $dollar constants {
                    $dollar ($dollar result_opcodes)*
                }
            };

            (
                @CONSTANTS $dollar constants:ident
                {
                    $dollar opcode:ident
                    $dollar ($dollar rest_of_opcodes:tt)*
                } -> { $dollar previous_opcode:ident $dollar ($dollar result_opcodes:tt)* }
            ) => {
                enumerate! {
                    @CONSTANTS $dollar constants
                    { $dollar ($dollar rest_of_opcodes)* } -> {
                        $dollar opcode $dollar ($dollar result_opcodes)*
                        pub const $dollar opcode: $opcode_tag_type = $dollar previous_opcode + 1;
                    }
                }
            };

            (
                @CONSTANTS $dollar constants:ident
                {
                    $dollar opcode:ident
                    $dollar ($dollar rest_of_opcodes:tt)*
                } -> { }
            ) => {
                enumerate! {
                    @CONSTANTS $dollar constants
                    { $dollar ($dollar rest_of_opcodes)* } -> {
                        $dollar opcode
                        pub const $dollar opcode: $opcode_tag_type = 0;
                    }
                }
            };
        }

        $(opcode! {
            @CONSTANTS $opcode_constants
            #[allow(unused)]
            #[derive(Debug, Clone)]
            $opcode $({
                $(
                    // $(#[address] $($opcode_field_is_address)?)?
                    $opcode_field_name : $opcode_field_type,
                )+
            } -> {}
            []
            @BACKPATCH [])?
        })+

        $(opcode! {
            @CONSTANTS $value_constants
            #[allow(unused)]
            #[derive(Debug, Clone, Copy)]
            $value $({
                $(
                    // $(#[address] $($value_field_is_address)?)?
                    $value_field_name : $value_field_type,
                )+
            } -> {}
            []
            @BACKPATCH [])?
        })+

        opcode! {
            @CONSTANTS $value_constants
            #[allow(unused)]
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            $arena_index_name {
                index : $arena_index_type,
            } -> {}
            []
            @BACKPATCH []
        }

        $(opcode! {
            @CONSTANTS $value_constants
            #[allow(unused)]
            #[derive(Debug, Clone)]
            $garbage_collected $({
                $(
                    // $(#[address] $($garbage_collected_field_is_address)?)?
                    $garbage_collected_field_name : $garbage_collected_field_type,
                )+
            } -> {}
            []
            @BACKPATCH [])?
        })*

        enumerate! {
            @CONSTANTS $opcode_constants
            { $($opcode)+ } -> { }
        }

        enumerate! {
            @CONSTANTS $value_constants
            { $($value)+ $arena_index_name $($garbage_collected)* } -> { }
        }
    };
}

pub use crate::__bytecode as bytecode;
