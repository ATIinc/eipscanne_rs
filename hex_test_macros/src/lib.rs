pub mod prelude {
    pub use crate::assert_eq_hex;
    pub use pretty_hex::{config_hex, HexConfig};
}

#[macro_export]
macro_rules! assert_eq_hex {
    // Case when both expected and actual are provided, along with a config
    ($expected:expr, $actual:expr, $config:expr) => {
        assert_eq!(
            $expected,
            $actual,
            "\n\nValues don't match in hex!\nExpected:\n-------\n{} \n\nActual:\n------- \n{}\n",
            config_hex(&$expected, $config),
            config_hex(&$actual, $config)
        );
    };

    // Case when only expected and actual are provided
    ($expected:expr, $actual:expr) => {
        const HEX_FORMAT_CONFIG: HexConfig = HexConfig {
            title: false,
            ascii: true,
            width: 30,
            group: 0,
            chunk: 1,
            max_bytes: usize::MAX,
            display_offset: 0,
        };

        // Call the first macro clause, passing the default config
        assert_eq_hex!($expected, $actual, HEX_FORMAT_CONFIG)
    };
}
